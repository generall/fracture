#!/usr/bin/env bash
# Native macOS build, or Linux cross-build with an existing macOS SDK.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
if [[ $(uname -s) == Linux ]]; then
    : "${SDKROOT:?Set SDKROOT to an extracted macOS SDK}"
    export SDKROOT
    export FRACTURE_MACOS_CLANG="${FRACTURE_MACOS_CLANG:-clang}"
    export FRACTURE_MACOS_LD="${FRACTURE_MACOS_LD:-$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/gcc-ld/ld64.lld}"
    export FRACTURE_MACOS_SDK_VERSION
    FRACTURE_MACOS_SDK_VERSION=$(python3 -c 'import os,plistlib; print(plistlib.load(open(os.path.join(os.environ["SDKROOT"],"SDKSettings.plist"),"rb"))["Version"])')
    mkdir -p target/apple-arm-tools
    cat > target/apple-arm-tools/clang <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
exec "$FRACTURE_MACOS_CLANG" --target="arm64-apple-macos$MACOSX_DEPLOYMENT_TARGET" -isysroot "$SDKROOT" "$@"
WRAPPER
    cat > target/apple-arm-tools/linker <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
exec "$FRACTURE_MACOS_CLANG" --target="arm64-apple-macos$MACOSX_DEPLOYMENT_TARGET" -isysroot "$SDKROOT" --ld-path="$FRACTURE_MACOS_LD" "-Wl,-platform_version,macos,$MACOSX_DEPLOYMENT_TARGET,$FRACTURE_MACOS_SDK_VERSION" "$@"
WRAPPER
    chmod +x target/apple-arm-tools/{clang,linker}
    export CC_aarch64_apple_darwin="$PWD/target/apple-arm-tools/clang"
    export AR_aarch64_apple_darwin="${AR_aarch64_apple_darwin:-llvm-ar}"
    export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER="$PWD/target/apple-arm-tools/linker"
fi
cargo build --release --locked --target aarch64-apple-darwin -p game -p sandbox
python3 tools/package-macos-arm64.py
