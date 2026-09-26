# Local game mode

Run `cargo run --release --locked -p game` or open `/` in the browser for the Fracture title screen. Choose **Enter Local Lobby** to play a match against one AI opponent. Desktop network play uses **Play on local network**; see [LAN multiplayer](LAN_MULTIPLAYER.md).

## Lobby

- Choose **Quarry Crossing**, **River Valley**, the existing **Editor Draft**, or **Your Map**. The preview shows the selected battlefield.
- **Create Map** opens a fresh map builder with both teams' spawns and flags. **Edit Selected Map** opens a copy of the selected recipe. **Use Map** or **M / Lobby** returns to setup; neither action starts gameplay.
- **Import JSON** accepts existing level files. The builder's File menu can export maps.
- Choose **Blue / Red**, **5 minutes / 10 minutes / Unlimited**, and an AI that **captures the flag** or **hunts the player**.
- **Start Match** checks the map, both flags, and both teams' physical spawn clearance. Invalid setups stay in the lobby with an error.

Your custom match map saves separately from the sandbox draft: `fracture.match-map.v1` in browser local storage, or `levels/match-map.json` on desktop. The lobby currently has one custom-map slot; export additional maps as JSON. Choosing a built-in map or editing a copy never overwrites the sandbox's `fracture.level.v1` / `levels/draft.json`.

## Match

A three-second countdown precedes simulation and firing. Drive with **WASD**, aim with the **mouse** or rotate the turret left/right with **Q / E** or **J / K**, fire with **Space / left-click / F**, switch HE/AP with **5 / 6 / B**, and use collected specials with **G / right-click**. **H** shows controls. The HUD shows hull condition, ammunition, special charges/charge-up, the match clock, and each team's tanks lost. Fog of war is always enabled.

Touch the enemy flag to win. Tank destruction uses the existing three-second respawn; it does not end a match. A time-limited match ends in a draw if neither flag is captured. The clock advances with simulation ticks and stops during the countdown, pause, and results. A capture on the final tick takes precedence over the time limit.

**Esc** opens the local pause menu with Resume, Restart Match, and Return to Lobby. Results offer Rematch and Return to Lobby. A rematch restores the original map, tanks, pickups, clock and statistics. Sandbox scene changes, editor shortcuts, reset, slow motion, single stepping, stress view and fog toggles are disabled in matches.

## Sound

The march crossfades into a faster battle theme when an enemy is visible. **Sound Settings / F7** opens saved master, music, effects and interface-cue sliders. **F8** mutes; **F9 / F10** adjust music/effects. Opening settings pauses gameplay until closed. See [sound design](AUDIO.md).

## Development access and ownership

The development tools have their own executable: `cargo run --release --locked -p sandbox`. In the browser, `/sandbox.html` loads `sandbox.wasm`; `/` loads `game.wasm`. There is no in-app switch between them. Experiment links use `/sandbox.html?experiment=...`, and desktop experiment flags belong to the sandbox executable.

Both executables use the shared `fracture_client` library in `crates/sandbox/src/lib.rs`; their small entry points choose the application and window title. `crates/game` is the game launcher.

`sim::matchplay` owns validated match configuration, fixed-tick timing, results and losses. `sandbox/game` owns the local lobby, map preview, countdown, menus and HUD. `Lab` continues to own the simulation and transient effects. Session rules contain no window, browser, UI or transport calls and are also used by the authoritative host in `crates/net`.

Run `node tools/game-smoke.mjs` with the local server and isolated Chrome setup described in CONTRIBUTING.md. It exercises creation and separate persistence, configuration, countdown/input gating, disabled sandbox shortcuts, pause/resume, invalid maps, actual flag captures, victory/defeat, rematches and saved-map restoration. Existing browser suites load the separate sandbox page.
