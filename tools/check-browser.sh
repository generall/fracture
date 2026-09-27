#!/usr/bin/env bash
# Run suites sequentially: editor/comparison checks share local browser storage.
# Requires the local server and an isolated Chrome CDP session (see CONTRIBUTING).
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
if [[ $# -eq 0 ]]; then
    set -- audio game controls deck browser editor comparison library plasticity barrel crush barrel-shot ammunition terrain level level-improvements demo
fi
# Validate the whole list before opening any tabs.
for suite in "$@"; do
    case "$suite" in
        audio|game|controls|deck|browser|editor|comparison|library|plasticity|barrel|crush|barrel-shot|ammunition|terrain|level|level-improvements|demo) ;;
        *) printf 'Unknown browser suite: %s\n' "$suite" >&2; exit 2 ;;
    esac
done
for suite in "$@"; do
    printf '\nRunning %s browser checks\n' "$suite"
    node "tools/${suite}-smoke.mjs"
done
