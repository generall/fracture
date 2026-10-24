#!/usr/bin/env bash
# Compatibility entry point; the shared builder supports Intel Macs too.
set -euo pipefail
exec bash "$(dirname -- "${BASH_SOURCE[0]}")/build-macos.sh" arm64 "$@"
