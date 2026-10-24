#!/usr/bin/env bash
# Portable Linux and Steam Deck release packages, including checks and notices.
set -euo pipefail
exec bash "$(dirname -- "${BASH_SOURCE[0]}")/build-release.sh" linux "$@"
