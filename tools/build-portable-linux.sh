#!/usr/bin/env bash
# Build on the manylinux glibc 2.28 baseline instead of the host's libc.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
source tools/release-config.sh
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    printf 'The portable Linux build needs x86_64 Linux and Docker.\n' >&2
    exit 1
fi
bash tools/fetch-release-sdl.sh
mkdir -p target/release-tools
# Build inputs are read-only; all compiler and package output stays in target/.
docker run --rm \
    --mount "type=bind,src=$PWD,dst=/src,readonly" \
    --mount "type=bind,src=$PWD/target,dst=/src/target" \
    --env "FRACTURE_BUILD_UID=$(id -u)" --env "FRACTURE_BUILD_GID=$(id -g)" \
    --env "FRACTURE_BUILD_JOBS=${FRACTURE_BUILD_JOBS:-4}" \
    --workdir /src "$FRACTURE_MANYLINUX_IMAGE" bash tools/build-portable-linux-inner.sh
