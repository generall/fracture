#!/usr/bin/env bash
# Use SDL's official universal framework, verified before extraction.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
source tools/release-config.sh
arch=${1:?Specify arm64 or x86_64}
cache="$PWD/target/release-tools"
archive="$cache/SDL2-$FRACTURE_SDL_VERSION.dmg"
if [[ ! -f "$archive" ]]; then
    curl --fail --location --retry 3 \
        "https://github.com/libsdl-org/SDL/releases/download/release-$FRACTURE_SDL_VERSION/SDL2-$FRACTURE_SDL_VERSION.dmg" \
        -o "$archive.tmp"
    mv "$archive.tmp" "$archive"
fi
python3 - "$archive" "$FRACTURE_SDL_MACOS_SHA256" <<'PY'
import hashlib, pathlib, sys
if hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest() != sys.argv[2]:
    raise SystemExit('SDL macOS framework checksum mismatch')
PY
extract="$cache/sdl-macos-framework"
if [[ ! -f "$extract/SDL2/SDL2.framework/Versions/A/SDL2" ]]; then
    mkdir -p "$extract"
    if [[ $(uname -s) == Darwin ]]; then
        mountpoint="$cache/sdl-macos-mount"
        mkdir -p "$mountpoint"
        hdiutil attach "$archive" -readonly -nobrowse -mountpoint "$mountpoint"
        trap 'hdiutil detach "$mountpoint"' EXIT
        mkdir -p "$extract/SDL2"
        cp -R "$mountpoint/SDL2.framework" "$extract/SDL2/"
        hdiutil detach "$mountpoint"
        trap - EXIT
    else
        7z x -y "-o$extract" "$archive" 'SDL2/SDL2.framework/Versions/A/SDL2' >/dev/null
    fi
fi
python3 - "$extract/SDL2/SDL2.framework/Versions/A/SDL2" "$arch" "$cache" <<'PY'
from pathlib import Path
import struct, sys
data = Path(sys.argv[1]).read_bytes()
cpu = {'arm64': 0x0100000C, 'x86_64': 0x01000007}[sys.argv[2]]
magic, count = struct.unpack('>II', data[:8])
assert magic == 0xCAFEBABE, 'Expected an official universal Mach-O framework'
for index in range(count):
    architecture, _, offset, size, _ = struct.unpack_from('>IIIII', data, 8 + index * 20)
    if architecture == cpu:
        output = Path(sys.argv[3]) / ('sdl-macos-' + sys.argv[2]) / 'libSDL2.dylib'
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(data[offset:offset + size])
        break
else:
    raise SystemExit('Requested architecture is missing from the SDL framework')
PY
