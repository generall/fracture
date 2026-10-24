# Steam Deck and gamepads

The game has a handheld layout for 1280×800 and 1280×720, controller navigation
for game menus, local matches, LAN lobbies and sound settings, and analog tank
controls. Keyboard/mouse controls remain available. Map/object editors and native
file import dialogs still need a pointer; use the Deck trackpad or a mouse.

## Play on Deck

Download the Steam Deck archive from [the latest release](https://github.com/generall/fracture/releases/latest).
In Desktop Mode, extract it and run `./install.sh`. Add the installed
`~/.local/opt/fracture/fracture-deck.sh` as a non-Steam game and select Steam Input's
**Gamepad** template. Launch it from Gaming Mode. This is a native Linux executable;
leave Proton disabled. The Deck launcher selects fullscreen 1280×800 automatically.
`SteamDeck=1` also selects this startup configuration. The layout follows the
actual window size, including 720p and docked displays. F11 toggles fullscreen.

To build locally on x86_64 Linux with Docker, run:

```sh
bash tools/build-release.sh linux
```

The versioned Linux and Deck archives are written to `target/dist/`.
See [local builds and releases](RELEASING.md) for the complete release process.

The launcher saves to `$XDG_DATA_HOME/fracture`, defaulting to
`~/.local/share/fracture`. Override with `FRACTURE_DATA_DIR`. Maps, drafts, audio
settings and checkpoints stay there when replacing the installation. Direct
`cargo run` keeps the existing working-directory save behavior unless that
variable is set. Existing development saves are not automatically moved.

The package embeds game assets and bundles SDL2 controller support. It uses the
manylinux 2.28 build baseline, with an audited maximum glibc requirement of 2.28.
Linux also needs X11/XWayland (libX11, libXi, libxkbcommon), OpenGL, ALSA and
libgcc_s, supplied by a normal SteamOS desktop installation. The release builder
runs silent installation and launch checks in that baseline container.
For a Steam store distribution, build and test inside Valve's
[Steam Runtime SDK](https://github.com/ValveSoftware/steam-runtime) and configure
the matching Steam Linux Runtime in Steamworks. The download release process
does not publish a Steam depot.

## Controls

| Deck / standard gamepad | Action |
| --- | --- |
| Left stick | Drive forward/reverse and steer |
| Right stick | Aim turret; mortar: up/down gradually adjusts range, release to hold |
| RT | Fire main gun |
| LT | Activate collected special weapon |
| RB | Change HE/AP ammunition |
| D-pad up/down in battle | Zoom |
| Menu / Start | Pause / match menu |
| X | Show/hide help |
| Y | Sound settings |
| D-pad / left stick in menus | Move focus |
| A | Activate focused control |
| B | Back / resume |

Selecting a LAN name or address with A opens a controller-operated keyboard.
In Deck mode, tapping the field or clicking it with the trackpad opens it too.
It supports letters, numbers and address punctuation, shift, delete and cancel.
Physical keyboard input also works. In **Deck mode** (`--deck` or `SteamDeck=1`),
the left stick only drives during a match; it navigates controls when a menu is
open. Controller use in this mode hides the mouse cursor
and aim crosshair, and ignores mouse motion so Steam's emulated pointer cannot
redirect the turret or highlight HUD buttons. Fire inputs (left click, Space,
F and G) retain controller aim, including Steam's emulated inputs. Right/middle
click or a keyboard key other than those fire keys switches back to the pointer;
pointer-based editors keep their cursor visible.
Normal desktop launches retain their existing cursor and mouse-motion handoff.
Prompts use Deck/Xbox button names; PlayStation-specific glyphs
and in-game remapping are not implemented (Steam Input can remap buttons).

During controller aiming, the camera smoothly looks ahead along the gun's actual
direction and keeps the vehicle visible near the rear of the view. Releasing the
right stick retains that view. The mortar camera follows its hull direction,
independently of stick range adjustment. D-pad up/down still controls zoom.

Sticks use a radial dead zone. A newly connected controller must return to
neutral before accepting input. Disconnecting during a match opens its menu
and clears pending local input. Local simulation pauses; LAN matches continue
on the host with neutral input from this player. Minimization and a frame gap
over one second also stop local play. Actual Deck sleep/wake behavior still needs
hardware validation. One controller drives the local player; this is not split-screen.

## Validation

### Physical device smoke test (2026-09-27)

The packaged native build was installed and launched on a Steam Deck running
SteamOS 3.8.28 in KDE Desktop Mode. All linked libraries resolved without installing
system packages. SDL identified the built-in controller as `Steam Deck Controller`.
The fullscreen X11 window measured 1280×800; a captured local match showed the
compact HUD, controller prompts and the tank away from its spawn position.
The game remained running while its window was inspected remotely.

On that device the installation is `~/Games/fracture`, with a **Fracture** entry
in the desktop application launcher. Saves use `~/.local/share/fracture`.
The development device is reachable with `ssh deck@192.168.0.10` (verified
2026-10-18). Its hostname is `steamdeck`.
For remote launches, use a transient user service (`systemd-run --user`) with
`DISPLAY` and `XAUTHORITY` from the active `plasmashell` session. This keeps the
game running after SSH disconnects. The current launch unit is
`fracture-game.service`; inspect it with `systemctl --user status fracture-game`
and `journalctl --user -u fracture-game`. Player launches use normal saved audio
settings; internal launch checks still require `FRACTURE_SILENT=1`.
The screenshot from this check is kept locally at
`target/steamdeck/device-match-1280x800.png`.

MangoHud recorded a 90-second local-match sample after manually starting logging
(its automatic start did not produce a log). Across 899 samples at approximately
100 ms intervals, the mean reported FPS was 90.4 and the median was 89.9. Eight
samples were below 60 FPS; the lowest was 21.4. The sampled frame-time p95 was
12.84 ms and p99 was 14.01 ms. These are periodically sampled readings, not a
complete per-frame trace or a computed 1% low. A second screenshot showed the
match continuing with broken cover and respawns. Raw timing data and its summary
are kept locally in `target/steamdeck/device-timing.csv` and
`target/steamdeck/device-timing-summary.json`.

This confirms native launch, rendering and a short gameplay timing sample on the
device. Per-button behavior and audible output still need the player's
confirmation. Gaming Mode/Steam overlay, sustained destruction-heavy performance,
sleep/wake and dock/undock remain separate checks.

### Automated checks

```sh
cargo test --workspace --locked
# Requires an installed SDL2 with virtual joystick support; no physical pad.
cargo test -p sandbox --lib virtual_controller_axes_buttons_and_disconnect -- --ignored
bash tools/build-web.sh
# Start tools/serve.py and isolated Chrome CDP as described in CONTRIBUTING.md.
bash tools/check-browser.sh deck game controls
bash tools/check-browser.sh camera-aim
```

`deck` selects the browser's `?deck=1` preview, injects a standard browser gamepad
and exercises menus, settings, analog
movement/aiming, firing, ammunition, pause and reconnect behavior at both handheld
resolutions. It also checks that normal desktop mode keeps a visible cursor and
mouse-motion handoff. Screenshots are written to `target/screenshots/deck-*.png`.
SDL's virtual-device test separately checks the native backend. Browser gamepad
access requires localhost or HTTPS; browser audio may require a click/key first.

`camera-aim` checks the actual gun-directed camera with a virtual gamepad: all
four aim directions at 1280×800 and 1280×720, released-stick retention despite
emulated mouse motion, scout aiming, and mortar hull direction during range
adjustment. It verifies extra forward reach and that the vehicle stays on screen.

Before a Steam release, test on a physical Deck: built-in controls and Steam
overlay, cold launch, all LAN flows, audio, sleep/wake, dock/undock, and sustained
frame pacing in a debris-heavy battle. The simulation remains at 120 Hz; no
performance claim or Steam Deck Verified status is implied by the desktop tests.
Valve's [compatibility requirements](https://partner.steamgames.com/doc/steamhardware/compat)
are the release checklist. Editors and file dialogs remain the main pointer-only
parts of the application.
