#!/usr/bin/env python3
"""Install and launch the real Linux packages under an isolated, silent X server."""
import os
from pathlib import Path
import select
import subprocess
import tarfile
import tempfile
import time

root = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory(prefix="fracture-release-") as temporary:
    temporary = Path(temporary)
    read_fd, write_fd = os.pipe()
    xserver = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0", "1280x800x24", "-nolisten", "tcp"],
                               pass_fds=(write_fd,), stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    os.close(write_fd)
    try:
        if not select.select([read_fd], [], [], 15)[0]:
            raise RuntimeError("Xvfb did not start")
        display = os.read(read_fd, 100).decode().strip()
        if not display or xserver.poll() is not None:
            raise RuntimeError("Xvfb failed: " + xserver.stderr.read().decode())
        # Null output is isolated to these test processes; the player's mixer is untouched.
        alsa = temporary / "alsa.conf"
        alsa.write_text('pcm.!default { type null }\n')
        env = {**os.environ, "DISPLAY": ":" + display, "FRACTURE_SILENT": "1",
               "ALSA_CONFIG_PATH": str(alsa), "LIBGL_ALWAYS_SOFTWARE": "1",
               "XDG_DATA_HOME": str(temporary / "user-data")}
        env.pop("FRACTURE_DATA_DIR", None)
        for platform in ("linux", "steamdeck"):
            # The caller builds one current version; never silently test an older release.
            import importlib.util
            spec = importlib.util.spec_from_file_location("packaging", root / "tools/package-release.py")
            packaging = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(packaging)
            archive = root / "target/dist" / f"fracture-{packaging.version()}-{platform}-x86_64.tar.gz"
            extract = temporary / platform
            with tarfile.open(archive) as package:
                package.extractall(extract, filter="data")
            source = next(extract.iterdir())
            install = temporary / f"installed {platform}"
            subprocess.run(["bash", str(source / "install.sh"), str(install)], env=env, check=True)
            desktop = (temporary / "user-data/applications/fracture.desktop").read_text()
            default_launcher = "fracture-deck.sh" if platform == "steamdeck" else "fracture.sh"
            assert str(install / default_launcher) in desktop
            # Reinstallation must preserve existing player data.
            saves = temporary / "user-data/fracture"
            saves.mkdir(exist_ok=True)
            sentinel = saves / "save-preservation-test"
            sentinel.write_text("player data")
            mixer = saves / "audio-settings.txt"
            mixer.write_text(str(0x40000000 | (73 << 1) | (41 << 8) | (82 << 15) | (57 << 22)))
            mixer_before = mixer.read_bytes()
            subprocess.run(["bash", str(source / "install.sh"), str(install)], env=env, check=True)
            assert sentinel.read_text() == "player data"
            for launcher in (default_launcher, "sandbox.sh"):
                with tempfile.TemporaryFile() as log:
                    game = subprocess.Popen([str(install / launcher)], cwd=temporary, env=env, stdout=log, stderr=log)
                    try:
                        time.sleep(4)
                        assert game.poll() is None, f"{platform}/{launcher} exited {game.returncode}"
                        mappings = Path(f"/proc/{game.pid}/maps").read_text()
                        assert str(install / "lib/libSDL2-2.0.so.0") in mappings, "Bundled controller runtime was not loaded"
                    finally:
                        game.terminate()
                        try:
                            game.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            game.kill()
                            game.wait()
                        log.seek(0)
                        output = log.read().decode(errors="replace")
                        print(output)
                        assert "panicked at" not in output, f"Panic in {platform}/{launcher}"
                print(f"Passed: installed {platform}/{launcher}, graphics, bundled SDL2, separate saves")
                assert mixer.read_bytes() == mixer_before, "Silent test changed saved mixer settings"
    finally:
        os.close(read_fd)
        xserver.terminate()
        xserver.wait(timeout=5)
