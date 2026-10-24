#!/usr/bin/env bash
set -euo pipefail
game_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export FRACTURE_DATA_DIR="${FRACTURE_DATA_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/fracture}"
exec "$game_dir/sandbox" "$@"
