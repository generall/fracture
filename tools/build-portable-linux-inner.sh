#!/usr/bin/env bash
# Internal entry point, run only by build-portable-linux.sh inside Docker.
set -euo pipefail
source tools/release-config.sh
export CARGO_HOME=/src/target/release-tools/cargo
export RUSTUP_HOME=/src/target/release-tools/rustup
export CARGO_TARGET_DIR=/src/target/portable-linux
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_BUILD_JOBS="${FRACTURE_BUILD_JOBS:-4}"
export RUSTUP_TOOLCHAIN="$FRACTURE_RUST_VERSION"
trap 'chown -R "$FRACTURE_BUILD_UID:$FRACTURE_BUILD_GID" /src/target/release-tools /src/target/portable-linux /src/target/dist /src/target/package-stage 2>/dev/null || true' EXIT
dnf -y install alsa-lib-devel systemd-devel libusb1-devel libX11 libXi \
    libxkbcommon mesa-libGL xorg-x11-server-Xvfb mesa-dri-drivers
if [[ ! -x "$CARGO_HOME/bin/rustup" ]]; then
    curl --fail --location --retry 3 https://sh.rustup.rs -o /src/target/release-tools/rustup-init.sh
    sh /src/target/release-tools/rustup-init.sh -y --profile minimal --default-toolchain "$FRACTURE_RUST_VERSION" --no-modify-path
fi
rustup toolchain install "$FRACTURE_RUST_VERSION" --profile minimal
sdl_build=/src/target/release-tools/sdl-linux
cmake -S "/src/target/release-tools/SDL2-$FRACTURE_SDL_VERSION" -B "$sdl_build" \
    -DCMAKE_BUILD_TYPE=Release -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TEST=OFF \
    -DSDL_AUDIO=OFF -DSDL_VIDEO=OFF -DSDL_RENDER=OFF
cmake --build "$sdl_build" --parallel "$CARGO_BUILD_JOBS"
cargo build --release --locked -p game -p sandbox
python3 tools/package-release.py linux --sdl "$sdl_build/libSDL2-2.0.so.0"
python3 tools/package-release.py steamdeck --sdl "$sdl_build/libSDL2-2.0.so.0"
FRACTURE_SILENT=1 python3 tools/check-release-linux.py
