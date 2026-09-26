"""Package the separate Apple Silicon binaries without bundling an SDK."""
import hashlib
import os
from pathlib import Path
import shutil
import struct
import zipfile

root = Path(__file__).resolve().parent.parent
release = root / "target/aarch64-apple-darwin/release"
output = root / "target/dist"
package = output / "fracture-macos-arm64"
package.mkdir(parents=True, exist_ok=True)
for binary in ("game", "sandbox"):
    source = release / binary
    magic, cpu = struct.unpack("<II", source.read_bytes()[:8])
    if (magic, cpu) != (0xFEEDFACF, 0x0100000C):
        raise SystemExit(f"{source} is not a 64-bit ARM Mach-O executable")
    shutil.copy2(source, package / binary)
    (package / binary).chmod(0o755)

for title, binary in (("Play", "game"), ("Sandbox", "sandbox")):
    launcher = package / f"{title}.command"
    launcher.write_text(
        '#!/bin/bash\nset -euo pipefail\n'
        'cd -- "$(dirname -- "$0")"\n'
        f'exec "./{binary}" "$@"\n'
    )
    launcher.chmod(0o755)

minimum = os.environ.get("MACOSX_DEPLOYMENT_TARGET", "11.0")
(package / "README.txt").write_text(
    f"FRACTURE - Apple Silicon / macOS {minimum}+\n\n"
    "Extract this folder into a writable location.\n"
    "Double-click Play.command for the game, or Sandbox.command for the editor.\n"
    "From Terminal in this folder: ./game or ./sandbox\n\n"
    "The executables are separate. Maps, fonts and audio are embedded.\n"
    "Draft maps and sound settings save in this folder when using the launchers.\n\n"
    "LAN: choose Play on local network; host or join by IP/discovery.\n"
    "TCP 45771 carries gameplay; UDP 45770 carries discovery.\n\n"
    "This development build is not Developer ID signed or notarized.\n"
    "Cross-compiled and inspected on Linux; not runtime-tested on a Mac.\n"
)
licenses = package / "licenses"
licenses.mkdir(exist_ok=True)
for source, name in (
    ("crates/sandbox/assets/FONT-LICENSE", "FONT-LICENSE"),
    ("crates/sandbox/assets/audio/GeneralUser-GS-LICENSE.txt", "GeneralUser-GS-LICENSE.txt"),
    ("crates/sandbox/assets/audio/README.md", "AUDIO-CREDITS.md"),
    ("web/MACROQUAD-LICENSE-MIT", "MACROQUAD-LICENSE-MIT"),
    ("vendor/quad-snd/LICENSE-MIT", "QUAD-SND-LICENSE-MIT"),
):
    shutil.copy2(root / source, licenses / name)

archive = output / "fracture-macos-arm64.zip"
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for path in sorted(package.rglob("*")):
        if path.is_file():
            z.write(path, path.relative_to(output))
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
(output / "fracture-macos-arm64.zip.sha256").write_text(f"{digest}  {archive.name}\n")
print(f"Built {archive} ({archive.stat().st_size / 1024 / 1024:.1f} MiB)")
print(f"SHA-256 {digest}")
