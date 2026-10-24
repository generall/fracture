#!/usr/bin/env bash
# Build every platform locally. No GitHub Actions or hosted runner is used.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
source tools/release-config.sh
if [[ -n $(git status --porcelain) ]]; then
    printf 'Commit the intended release source before building; package provenance requires a clean tree.\n' >&2
    exit 1
fi
platform=${1:-all}
case "$platform" in
    all|linux|macos-arm64|macos-x86_64) ;;
    *) printf 'Usage: bash tools/build-release.sh [all|linux|macos-arm64|macos-x86_64]\n' >&2; exit 2 ;;
esac
if [[ $(uname -s) == Linux && "$platform" != linux ]]; then
    if [[ -z ${SDKROOT:-} && -d "$PWD/.tools/apple-arm/MacOSX11.3.sdk" ]]; then
        export SDKROOT="$PWD/.tools/apple-arm/MacOSX11.3.sdk"
    fi
    : "${SDKROOT:?Set SDKROOT to an extracted macOS SDK; see docs/RELEASING.md}"
fi
if [[ "$platform" == all && $(uname -s) != Linux ]]; then
    printf 'Build linux on an x86_64 Linux host; macOS targets can also build on a Mac.\n' >&2
    exit 1
fi
export RUSTUP_HOME="$PWD/target/release-tools/host-rustup"
export RUSTUP_TOOLCHAIN="$FRACTURE_RUST_VERSION"
export CARGO_BUILD_JOBS="${FRACTURE_BUILD_JOBS:-4}"
rustup toolchain install "$FRACTURE_RUST_VERSION" --profile minimal --component rustfmt --component clippy
python3 tools/release-licenses.py
# Keep the complete check gate in the normal release command.
FRACTURE_SILENT=1 bash tools/check.sh
if [[ "$platform" == all || "$platform" == linux ]]; then
    bash tools/build-portable-linux.sh
fi
if [[ "$platform" == all || "$platform" == macos-arm64 ]]; then
    rustup target add aarch64-apple-darwin
    bash tools/build-macos.sh arm64
fi
if [[ "$platform" == all || "$platform" == macos-x86_64 ]]; then
    rustup target add x86_64-apple-darwin
    bash tools/build-macos.sh x86_64
fi
printf 'Packages and SHA-256 files are in target/dist/.\n'
