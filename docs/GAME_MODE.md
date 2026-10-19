# Local game mode

Run `cargo run --release --locked -p game` or open `/` in the browser for the Fracture title screen. Choose **Enter Local Lobby** to play a match against one AI opponent. Desktop network play uses **Play on local network**; see [LAN multiplayer](LAN_MULTIPLAYER.md).

The opponent uses human visibility and control rules and remembers its own
sightings. Recorded four-minute 2v2 bot matches and winner/challenger development
are described in [AI arena](AI_ARENA.md).

## Lobby

- Choose **River Valley** (`levels/dota-level.json`) or **Classic Tanks 2** (`levels/classic-tanks-2.json`). These are the two bundled game maps; the preview shows the selected battlefield.
- **Import Map** accepts existing level JSON files and adds an **Imported Map** slot. Create and export maps in the separate sandbox application; the game lobby has no map editor.
- Choose **Blue / Red**, **5 minutes / 10 minutes / Unlimited**, and an AI that **captures the flag** or **hunts the player**.
- **Start Match** checks the map, both flags, and both teams' physical spawn clearance. Invalid setups stay in the lobby with an error.

Your custom match map saves separately from the sandbox draft: `fracture.match-map.v1` in browser local storage, or `levels/match-map.json` on desktop. The lobby currently has one custom-map slot; export additional maps as JSON. Selecting or importing a match map never overwrites the sandbox's `fracture.level.v1` / `levels/draft.json`.

## Vehicles

Deployment offers three distinct silhouettes: a wheeled scout car, the regular
tracked tank, and a broad tracked mortar carrier with an open fighting compartment.

| Vehicle | Ground speed / mass | Main weapon | Sight |
| --- | --- | --- | --- |
| Scout car | 10 m/s / 1.2 t; car steering, no pivot or turn braking | Automatic small gun, 12 HP per hit, 0.18 s interval; 1° shot spread kick, 8° maximum, 0.2 s aim-settling time. Gun damages vehicles only. No specials. | 24 m / 360°, independent of gun direction |
| Regular tank | 6.5 m/s / 4.5 t | Existing rotating HE/AP gun and special pickups | 24 m / 90° cone, 3 m nearby circle |
| Mortar | 3.6 m/s / 16 t; gradual acceleration, extra pushing force against breakable walls | Fixed gun, 3 s reload; arcing rounds pass over cover, with 75% of rocket blast energy and material shock, about 5 m blast radius | 36 m / 45° cone, 6 m nearby circle |

Scout impacts use the same contact and fracture physics as other vehicles.
The light chassis and lower engine force make it a poor bulldozer: a fast crash
can chip weak masonry, transfer momentum and damage the car. Velocity and spin
are never cancelled just because a wall is nearby. Its lower top speed reduces
crash energy, while it remains faster than the regular tank.

Mortar direction follows the hull. Its impact circle expands while moving or turning and tightens when stationary; shells scatter within the displayed circle, including range error. At 20 m, settled aim is about 0.4 m radius, versus 3.2 m at cruising speed. Mouse distance selects a range from 6–36 m;
Q/E or J/K adjust range without a mouse. On a controller, hold the right stick up/down to extend/shorten range gradually.
Small deflections give fine adjustment; releasing the stick retains the chosen range. The landing marker remains visible with controller input. Terrain
continues to affect traction and sight. The scout has lighter armor; the mortar
has heavier armor. Human choices apply independently in local and LAN matches.

Automated turrets can be authored in the sandbox's level editor. They have a
Blue, Red or Neutral ownership, a facing direction, configurable health and firing arc. Neutral turrets attack both teams; living friendly turrets share their vision, respecting cover and foliage. Destroy enemy turrets
before capturing them: stay within 1 m of the rear repair console for 15 seconds
to rebuild for your team. Leaving resets progress; an opposing tank contests it.
Repairs restore the configured health pool. If capture is disabled, only the owning team may repair; neutral ruins stay destroyed. Turret destruction does
not count as a player vehicle loss. Fixed guns transfer ramming force into their
foundations: ram damage is one eighth of the ordinary tank value, while gunfire
and explosive damage retain their existing armor rules.

## Match

A three-second countdown precedes simulation and firing. Drive with **WASD**, aim with the **mouse** or rotate the turret left/right with **Q / E** or **J / K**, fire with **Space / left-click / F**, switch HE/AP with **5 / 6 / B**, and use collected specials with **G / right-click**. **H** shows controls. The battlefield fills the screen. A small timer sits at the top, and a special-weapon overlay appears only while carrying or using a special: charges, readiness, reload or active/charge-up time. Main-gun reload stays on the aiming indicator; controls and losses appear in help and match menus. Fog of war is always enabled.

Touch the enemy flag to win. Choose a vehicle at initial deployment and after every death. Respawning requires both a selection and the ten-second minimum delay, with a clear spawn. The match continues while choosing; destruction does not end a match. A time-limited match ends in a draw if neither flag is captured. The clock advances with simulation ticks and stops during the countdown, pause, and results. A capture on the final tick takes precedence over the time limit.

**Esc** opens the local pause menu with Resume, Restart Match, and Return to Lobby. Results offer Rematch and Return to Lobby. A rematch restores the original map, tanks, pickups, clock and statistics. Sandbox scene changes, editor shortcuts, reset, slow motion, single stepping, stress view and fog toggles are disabled in matches.

## Sound

The march crossfades into a faster battle theme when an enemy is visible. **Sound Settings / F7** opens saved master, music, effects and interface-cue sliders. **F8** mutes; **F9 / F10** adjust music/effects. Opening settings pauses gameplay until closed. See [sound design](AUDIO.md).

## Development access and ownership

The development tools have their own executable: `cargo run --release --locked -p sandbox`. In the browser, `/sandbox.html` loads `sandbox.wasm`; `/` loads `game.wasm`. There is no in-app switch between them. Experiment links use `/sandbox.html?experiment=...`, and desktop experiment flags belong to the sandbox executable.

Both executables use the shared `fracture_client` library in `crates/sandbox/src/lib.rs`; their small entry points choose the application and window title. `crates/game` is the game launcher.

`sim::matchplay` owns validated match configuration, fixed-tick timing, results and losses. `sandbox/game` owns the local lobby, map preview, countdown, menus and HUD. `Lab` continues to own the simulation and transient effects. Session rules contain no window, browser, UI or transport calls and are also used by the authoritative host in `crates/net`.

Run `node tools/game-smoke.mjs` with the local server and isolated Chrome setup described in CONTRIBUTING.md. It exercises bundled map selection, imports and separate persistence, configuration, countdown/input gating, disabled sandbox shortcuts, pause/resume, invalid maps, actual flag captures, victory/defeat, rematches and saved-map restoration. Existing browser suites load the separate sandbox page.
