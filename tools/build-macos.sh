#!/usr/bin/env bash
# Native macOS or Linux cross-build using an existing macOS SDK.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
source tools/release-config.sh
arch=${1:-arm64}
case "$arch" in
    arm64) target=aarch64-apple-darwin; clang_arch=arm64 ;;
    x86_64) target=x86_64-apple-darwin; clang_arch=x86_64 ;;
    *) printf 'Usage: bash tools/build-macos.sh [arm64|x86_64]\n' >&2; exit 2 ;;
esac
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS="${FRACTURE_BUILD_JOBS:-4}"
bash tools/fetch-release-sdl.sh
sdl_build="$PWD/target/release-tools/sdl-macos-$arch"
bash tools/fetch-macos-sdl.sh "$arch"
if [[ $(uname -s) == Linux ]]; then
    : "${SDKROOT:?Set SDKROOT to an extracted macOS SDK (see docs/RELEASING.md)}"
    SDKROOT=$(cd -- "$SDKROOT" && pwd)
    export SDKROOT
    export FRACTURE_MACOS_CLANG="${FRACTURE_MACOS_CLANG:-clang}"
    export FRACTURE_MACOS_LD="${FRACTURE_MACOS_LD:-$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/gcc-ld/ld64.lld}"
    export FRACTURE_MACOS_ARCH="$clang_arch"
    export FRACTURE_MACOS_SDK_VERSION
    FRACTURE_MACOS_SDK_VERSION=$(python3 -c 'import os,plistlib; print(plistlib.load(open(os.path.join(os.environ["SDKROOT"],"SDKSettings.plist"),"rb"))["Version"])')
    wrapper="$PWD/target/release-tools/apple-$arch"
    mkdir -p "$wrapper"
    cat > "$wrapper/clang" <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
exec "$FRACTURE_MACOS_CLANG" --target="$FRACTURE_MACOS_ARCH-apple-macos$MACOSX_DEPLOYMENT_TARGET" \
    -isysroot "$SDKROOT" --ld-path="$FRACTURE_MACOS_LD" \
    "-Wl,-platform_version,macos,$MACOSX_DEPLOYMENT_TARGET,$FRACTURE_MACOS_SDK_VERSION" "$@"
WRAPPER
    chmod +x "$wrapper/clang"
    target_env=${target//-/_}
    export "CC_$target_env=$wrapper/clang"
    export "AR_$target_env=${FRACTURE_MACOS_AR:-llvm-ar-14}"
    linker_env=$(printf '%s' "$target_env" | tr '[:lower:]' '[:upper:]')
    export "CARGO_TARGET_${linker_env}_LINKER=$wrapper/clang"
elif [[ $(uname -s) != Darwin ]]; then
    printf 'Build on macOS or Linux with a macOS SDK.\n' >&2
    exit 1
fi
cargo build --release --locked --target "$target" -p game -p sandbox
python3 tools/package-release.py "macos-$arch" --sdl "$sdl_build/libSDL2.dylib"
