#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
cargo build --locked --release --target wasm32-unknown-unknown -p sandbox
cp target/wasm32-unknown-unknown/release/sandbox.wasm web/sandbox.wasm
printf 'Browser build ready. Run: python3 tools/serve.py\n'
