#!/usr/bin/env python3
"""Stage versioned desktop packages, audit native binaries, and write checksums."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "target/dist"
CONFIG = dict(re.findall(r"^([A-Z0-9_]+)=(.+)$", (ROOT / "tools/release-config.sh").read_text(), re.M))
LICENSES = {
    "crates/sandbox/assets/FONT-LICENSE": "FONT-LICENSE",
    "crates/sandbox/assets/audio/GeneralUser-GS-LICENSE.txt": "GeneralUser-GS-LICENSE.txt",
    "crates/sandbox/assets/audio/README.md": "AUDIO-CREDITS.md",
    "web/MACROQUAD-LICENSE-MIT": "MACROQUAD-LICENSE-MIT",
    "vendor/quad-snd/LICENSE-MIT": "QUAD-SND-LICENSE-MIT",
}


def version():
    manifest = (ROOT / "Cargo.toml").read_text()
    return re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"$', manifest, re.M)[1]


def copy(source, destination, executable=False):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(0o755 if executable else 0o644)


def audit_linux(path):
    header = path.read_bytes()[:20]
    if header[:5] != b"\x7fELF\x02" or struct.unpack("<H", header[18:20])[0] != 62:
        raise SystemExit(f"Not an x86_64 ELF: {path}")
    symbols = subprocess.check_output(["objdump", "-T", str(path)], text=True)
    versions = [tuple(map(int, v.split('.'))) for v in re.findall(r"GLIBC_([0-9.]+)", symbols)]
    if not versions or max(versions) > (2, 28):
        raise SystemExit(f"{path}: exceeds the glibc 2.28 baseline: {max(versions, default=None)}")
    dynamic = subprocess.check_output(["objdump", "-p", str(path)], text=True)
    allowed = {"libasound.so.2", "libgcc_s.so.1", "libc.so.6", "libm.so.6",
               "libdl.so.2", "libpthread.so.0", "librt.so.1", "ld-linux-x86-64.so.2"}
    needed = set(re.findall(r"NEEDED\s+(\S+)", dynamic))
    if needed - allowed:
        raise SystemExit(f"Unbundled Linux dependencies in {path}: {needed - allowed}")
    print(f"Audited {path.name}: glibc {'.'.join(map(str, max(versions)))}; {sorted(needed)}")


def audit_macos(path, architecture):
    expected = 0x0100000C if architecture == "arm64" else 0x01000007
    if struct.unpack("<II", path.read_bytes()[:8]) != (0xFEEDFACF, expected):
        raise SystemExit(f"Not a {architecture} Mach-O: {path}")
    data = path.read_bytes()
    commands = struct.unpack_from("<I", data, 16)[0]
    offset = 32
    for _ in range(commands):
        command, size = struct.unpack_from("<II", data, offset)
        if command in (0xC, 0x80000018, 0x8000001F):
            name_offset = struct.unpack_from("<I", data, offset + 8)[0]
            name = data[offset + name_offset:offset + size].split(b"\0")[0].decode()
            if not name.startswith(("/System/Library/", "/usr/lib/")):
                raise SystemExit(f"Unbundled macOS dependency: {name} in {path}")
        if command == 0x32:
            platform, minimum = struct.unpack_from("<II", data, offset + 8)
            if platform != 1 or minimum > 0x000B0000:
                raise SystemExit(f"{path} requires a newer macOS than 11.0")
        offset += size


def checksum(path):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    path.with_name(path.name + ".sha256").write_text(f"{digest}  {path.name}\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["linux", "steamdeck", "macos-arm64", "macos-x86_64"])
    parser.add_argument("--sdl", required=True, type=Path, help="built SDL2 shared library")
    args = parser.parse_args()
    package_name = f"fracture-{version()}-{args.platform}"
    if args.platform in ("linux", "steamdeck"):
        package_name += "-x86_64"
    stage = ROOT / "target/package-stage" / package_name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    DIST.mkdir(parents=True, exist_ok=True)
    commit = subprocess.check_output(
        ["git", "-c", f"safe.directory={ROOT}", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    metadata = {"version": version(), "commit": commit, "platform": args.platform,
                "sdl_version": CONFIG["FRACTURE_SDL_VERSION"],
                "rust_version": CONFIG["FRACTURE_RUST_VERSION"], "source_dirty": bool(subprocess.check_output(
                    ["git", "-c", f"safe.directory={ROOT}", "status", "--porcelain"], cwd=ROOT))}
    if args.platform.startswith("macos-"):
        arch = args.platform.removeprefix("macos-")
        target = "aarch64" if arch == "arm64" else "x86_64"
        release = ROOT / f"target/{target}-apple-darwin/release"
        minimum = os.environ.get("MACOSX_DEPLOYMENT_TARGET", "11.0")
        metadata["minimum_macos"] = minimum
        for title, binary in (("Fracture", "game"), ("Fracture Sandbox", "sandbox")):
            app = stage / f"{title}.app/Contents"
            executable = app / f"MacOS/{binary}"
            copy(release / binary, executable, True)
            audit_macos(executable, arch)
            library = app / "Frameworks/libSDL2.dylib"
            copy(args.sdl, library, True)
            audit_macos(library, arch)
            launcher = app / "MacOS/fracture"
            launcher.write_text(
                '#!/bin/bash\nset -euo pipefail\n'
                'app_dir=$(cd -- "$(dirname -- "$0")" && pwd)\n'
                'export FRACTURE_DATA_DIR="${FRACTURE_DATA_DIR:-$HOME/Library/Application Support/Fracture}"\n'
                f'exec "$app_dir/{binary}" "$@"\n'
            )
            launcher.chmod(0o755)
            with (app / "Info.plist").open("wb") as out:
                plistlib.dump({"CFBundleExecutable": "fracture", "CFBundleName": title,
                              "CFBundleDisplayName": title, "CFBundlePackageType": "APPL",
                              "CFBundleIdentifier": f"com.generall.fracture.{binary}",
                              "CFBundleVersion": version(), "CFBundleShortVersionString": version(),
                              "LSMinimumSystemVersion": minimum, "NSHighResolutionCapable": True}, out)
        readme = (
            f"Fracture {version()} — macOS {minimum}+ / {arch}\n\n"
            "Drag Fracture.app to Applications and double-click to play.\n"
            "Fracture Sandbox.app contains the editors and development tools.\n"
            "Saves: ~/Library/Application Support/Fracture (shared by both apps).\n"
            "Override with FRACTURE_DATA_DIR. Saves survive application updates.\n\n"
            "This build is not Developer ID signed or notarized. After trying to open it,\n"
            "use System Settings > Privacy & Security > Open Anyway if macOS blocks it.\n"
            "https://support.apple.com/102445\n"
            "Linux cross-builds are inspected but cannot be runtime-tested on Linux.\n"
        )
    else:
        release = ROOT / "target/portable-linux/release"
        metadata["minimum_glibc"] = "2.28"
        metadata["build_image"] = CONFIG["FRACTURE_MANYLINUX_IMAGE"]
        for binary in ("game", "sandbox"):
            copy(release / binary, stage / binary, True)
            audit_linux(stage / binary)
        library = stage / "lib/libSDL2-2.0.so.0"
        copy(args.sdl, library, True)
        audit_linux(library)
        for source, name in (("launch-linux.sh", "fracture.sh"),
                             ("launch-deck.sh", "fracture-deck.sh"),
                             ("launch-sandbox.sh", "sandbox.sh"),
                             ("install-linux.sh", "install.sh")):
            copy(ROOT / "tools" / source, stage / name, True)
        if args.platform == "steamdeck":
            (stage / "STEAMDECK").write_text("Launch fracture-deck.sh for handheld mode.\n")
        readme = (
            f"Fracture {version()} — Linux x86_64 / glibc 2.28+\n\n"
            "Run ./fracture.sh to play, ./fracture-deck.sh for Steam Deck,\n"
            "or ./sandbox.sh for editors. No Rust installation is needed.\n"
            "Optional installation: ./install.sh [destination]\n"
            "Default: ~/.local/opt/fracture, with a Fracture application menu entry.\n"
            "Run the new package's install.sh to update the same installation.\n"
            "Uninstall: delete the installation and ~/.local/share/applications/fracture.desktop.\n\n"
            "Steam Deck: in Desktop Mode, install then add fracture-deck.sh as a\n"
            "non-Steam game. Use Steam Input's Gamepad template. Leave Proton disabled.\n"
            "Return to Gaming Mode and launch from your library.\n\n"
            "Saves: $XDG_DATA_HOME/fracture (default ~/.local/share/fracture).\n"
            "Override with FRACTURE_DATA_DIR. Updates do not replace saves.\n"
            "Requirements: X11/XWayland (libX11, libXi, libxkbcommon), OpenGL, ALSA (libasound.so.2),\n"
            "libgcc_s.so.1, and glibc 2.28+. SDL2 controller support is bundled.\n"
        )
    (stage / "README.txt").write_text(readme + "\nLAN: TCP 45771; discovery: UDP 45770.\n")
    (stage / "BUILD.json").write_text(json.dumps(metadata, indent=2) + "\n")
    for source, name in LICENSES.items():
        copy(ROOT / source, stage / "licenses" / name)
    copy(ROOT / f'target/release-tools/SDL2-{CONFIG["FRACTURE_SDL_VERSION"]}/LICENSE.txt', stage / "licenses/SDL2-LICENSE.txt")
    copy(ROOT / "target/release-tools/THIRD-PARTY-LICENSES.txt", stage / "licenses/THIRD-PARTY-LICENSES.txt")
    if args.platform.startswith("macos-"):
        for app in stage.glob("*.app"):
            shutil.copytree(stage / "licenses", app / "Contents/Resources/licenses")
            copy(stage / "BUILD.json", app / "Contents/Resources/BUILD.json")
        if shutil.which("codesign"):
            for app in stage.glob("*.app"):
                subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
        archive = DIST / f"{package_name}.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as out:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    out.write(path, path.relative_to(stage.parent))
        with zipfile.ZipFile(archive) as check:
            if check.testzip() is not None:
                raise SystemExit("Corrupt zip archive")
    else:
        archive = DIST / f"{package_name}.tar.gz"
        with tarfile.open(archive, "w:gz") as out:
            out.add(stage, arcname=package_name)
    checksum(archive)
    print(f"Built {archive} ({archive.stat().st_size / 1024 / 1024:.1f} MiB)")


if __name__ == "__main__":
    main()
