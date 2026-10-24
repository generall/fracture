#!/usr/bin/env bash
# Publish locally built packages; never starts a GitHub Actions workflow.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(python3 -c 'import re,pathlib; print(re.search(r"^version = \"([0-9]+\.[0-9]+\.[0-9]+)\"$",pathlib.Path("Cargo.toml").read_text(),re.M)[1])')
tag=${1:-"v$version"}
if [[ "$tag" != "v$version" || $# -gt 1 ]]; then
    printf 'Usage: bash tools/publish-release.sh v%s (must match Cargo.toml)\n' "$version" >&2
    exit 2
fi
if [[ -n $(git status --porcelain) ]]; then
    printf 'Commit all release changes before publishing. Packages must identify the exact source commit.\n' >&2
    exit 1
fi
repo=generall/fracture
gh auth status
source_archive="target/dist/fracture-$version-source.tar.gz"
git archive --format=tar.gz --prefix="fracture-$version-source/" HEAD > "$source_archive"
python3 - "$source_archive" <<'PY'
import hashlib, pathlib, sys
path = pathlib.Path(sys.argv[1])
path.with_name(path.name + '.sha256').write_text(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n')
PY
python3 tools/verify-release.py
artifacts=("target/dist/fracture-$version-linux-x86_64.tar.gz"
    "target/dist/fracture-$version-steamdeck-x86_64.tar.gz"
    "target/dist/fracture-$version-macos-arm64.zip"
    "target/dist/fracture-$version-macos-x86_64.zip" "$source_archive")
checksums=()
for path in "${artifacts[@]}"; do checksums+=("$path.sha256"); done
notes="target/dist/release-notes-$version.md"
cat > "$notes" <<NOTES
Fracture $version: native packages built locally from $(git rev-parse HEAD).

| Platform | Download | Run / install |
| --- | --- | --- |
| Linux x86_64 | fracture-$version-linux-x86_64.tar.gz | Extract and run \`./fracture.sh\`; \`./install.sh\` adds an application menu entry. |
| Steam Deck | fracture-$version-steamdeck-x86_64.tar.gz | Extract in Desktop Mode, run \`./install.sh\`, add the installed \`fracture-deck.sh\` as a non-Steam game. Select Gamepad controls; leave Proton disabled. |
| macOS Apple Silicon | fracture-$version-macos-arm64.zip | Extract, drag Fracture.app to Applications and open it. |
| macOS Intel | fracture-$version-macos-x86_64.zip | Extract, drag Fracture.app to Applications and open it. |

Linux packages use the manylinux 2.28 build environment and require glibc 2.28+, X11/XWayland, OpenGL, ALSA and libgcc_s. These are native game archives, not Python wheels. SDL2 controller support is bundled. No Rust installation is needed. Each package includes the separate sandbox/editor application.

macOS requires 11.0+. These locally cross-compiled app bundles are not Developer ID signed or notarized and have not been runtime-tested on a Mac. If blocked after opening, use System Settings → Privacy & Security → Open Anyway. See [Apple's instructions](https://support.apple.com/102445).

Saves are outside the installation and survive upgrades. See the included README.txt. SHA-256 files and the exact source archive are attached. Steam Deck Gaming Mode, sleep/wake and full hardware validation remain manual checks; this is not a Steam store release or a Deck Verified claim.

For the next version, update the workspace version, commit, run \`bash tools/build-release.sh\`, then \`bash tools/publish-release.sh vX.Y.Z\`. See [the local release guide](https://github.com/$repo/blob/$tag/docs/RELEASING.md).
NOTES
if git rev-parse "$tag" >/dev/null 2>&1; then
    [[ $(git rev-list -n 1 "$tag") == $(git rev-parse HEAD) ]] || { printf 'Tag points at another commit.\n' >&2; exit 1; }
else
    git tag -a "$tag" -m "Fracture $version"
fi
git push origin "refs/tags/$tag"
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    [[ $(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft) == true ]] || {
        printf 'Refusing to replace an already published release.\n' >&2; exit 1;
    }
else
    gh release create "$tag" --repo "$repo" --verify-tag --draft --title "Fracture $version" --notes-file "$notes"
fi
gh release upload "$tag" --repo "$repo" --clobber "${artifacts[@]}" "${checksums[@]}"
gh release edit "$tag" --repo "$repo" --draft=false --latest --notes-file "$notes"
gh release view "$tag" --repo "$repo" --json url --jq .url
