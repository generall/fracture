#!/usr/bin/env bash
# Run inside the target Steam Runtime SDK for a Steam distribution build.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    printf 'Build on x86_64 Linux (Steam Deck target).\n' >&2
    exit 1
fi
cargo build --release --locked -p game
package=target/linux-x86_64/fracture
mkdir -p "$package"
install -m755 target/release/game "$package/game"
install -m755 tools/launch-linux.sh "$package/fracture.sh"
cp docs/STEAM_DECK.md "$package/README.md"
cp crates/sandbox/assets/FONT-LICENSE "$package/FONT-LICENSE"
tar -C target/linux-x86_64 -czf target/fracture-linux-x86_64.tar.gz fracture
printf 'Ready: target/fracture-linux-x86_64.tar.gz\n'
