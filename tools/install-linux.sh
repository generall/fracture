#!/usr/bin/env bash
# Copy this extracted package to a per-user installation; never needs root.
set -euo pipefail
package_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
destination=${1:-"$HOME/.local/opt/fracture"}
mkdir -p "$destination"
destination=$(cd -- "$destination" && pwd)
if [[ "$package_dir" != "$destination" ]]; then
    if [[ ! -f "$package_dir/STEAMDECK" ]]; then
        rm -f "$destination/STEAMDECK"
    fi
    cp -R "$package_dir/." "$destination/"
fi
applications="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$applications"
# Desktop Exec has its own escaping rules, separate from shell quoting.
desktop_escape() {
    local value=$1
    value=${value//\\/\\\\}
    value=${value//\"/\\\"}
    value=${value//\$/\\\$}
    value=${value//\`/\\\`}
    value=${value//%/%%}
    printf '%s' "$value"
}
if [[ -f "$destination/STEAMDECK" ]]; then
    launcher=fracture-deck.sh
else
    launcher=fracture.sh
fi
cat > "$applications/fracture.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Fracture
Comment=Tank combat and physical destruction
Exec="$(desktop_escape "$destination/$launcher")"
Icon=applications-games
Terminal=false
Categories=Game;ActionGame;
DESKTOP
printf 'Installed to %s\nLaunch Fracture from your application menu.\n' "$destination"
if [[ "$launcher" == fracture-deck.sh ]]; then
    printf 'Steam Deck: add %s/fracture-deck.sh as a non-Steam game. Use the Gamepad template; leave Proton disabled.\n' "$destination"
fi
