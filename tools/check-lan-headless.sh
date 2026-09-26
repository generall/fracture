#!/usr/bin/env bash
# Native LAN tests and the six-client large-map benchmark, without a display.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
unset DISPLAY WAYLAND_DISPLAY
cargo fmt --all -- --check
cargo test --workspace --locked
cargo test -p tank-net --locked --test lan -- --ignored --nocapture
cargo clippy --workspace --all-targets --locked -- -D warnings \
  -A clippy::chunks_exact_to_as_chunks \
  -A clippy::items_after_test_module \
  -A clippy::approx_constant
cargo run --release --locked -p tank-net --example lan_benchmark
