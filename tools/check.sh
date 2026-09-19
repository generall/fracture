#!/usr/bin/env bash
# Standard Rust checks. Add --web to also refresh the playable browser build.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
if [[ $# -gt 1 || ( $# -eq 1 && "${1:-}" != "--web" ) ]]; then
    printf 'Usage: bash tools/check.sh [--web]\n' >&2
    exit 2
fi
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if [[ ${1:-} == "--web" ]]; then
    bash tools/build-web.sh
fi
