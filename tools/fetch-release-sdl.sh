#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
source tools/release-config.sh
cache="$PWD/target/release-tools"
mkdir -p "$cache"
archive="$cache/SDL2-$FRACTURE_SDL_VERSION.tar.gz"
if [[ ! -f "$archive" ]]; then
    curl --fail --location --retry 3 \
        "https://github.com/libsdl-org/SDL/releases/download/release-$FRACTURE_SDL_VERSION/SDL2-$FRACTURE_SDL_VERSION.tar.gz" \
        -o "$archive.tmp"
    mv "$archive.tmp" "$archive"
fi
python3 - "$archive" "$FRACTURE_SDL_SHA256" <<'PY'
import hashlib, pathlib, sys
path = pathlib.Path(sys.argv[1])
if hashlib.sha256(path.read_bytes()).hexdigest() != sys.argv[2]:
    raise SystemExit(f"SDL checksum mismatch: {path}; remove it and retry")
PY
if [[ ! -d "$cache/SDL2-$FRACTURE_SDL_VERSION" ]]; then
    tar -xzf "$archive" -C "$cache"
fi
