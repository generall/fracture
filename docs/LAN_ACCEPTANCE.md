# LAN multiplayer acceptance — 2026-09-26

The desktop LAN implementation is complete. The requested headless checks passed
before continuing desktop checks. The current release binaries include the final
text-input correction described below.

## Requirement audit

| Requirement | Implementation and evidence |
| --- | --- |
| Host plus up to five guests | Dedicated host thread; six actual TCP clients in integration tests and benchmark. |
| IP and automatic LAN joining | Direct numeric address parsing and UDP discovery; real-socket coverage and both desktop join paths exercised. |
| Host selects map, sees teams and readiness | Desktop roster, presets, editor/import and duration controls; map revisions reset readiness; start requires both teams and every player ready. |
| Physics only on host | MultiplayerWorld owns all physics/combat; display replicas contain no physics bodies and cannot step simulation. Unit and socket tests cover independent controls, combat, specials and respawns. |
| Efficient binary updates | Bounded varint bincode/LZ4 frames, persistent geometry, changed quantized motion and vision; map transfer once per match. Delta re-entry and slow-reader recovery tested. |
| Per-player visibility | Shared blocker preparation and individual filtering of bodies, tanks, projectiles and events; privacy/re-entry and exact cache equivalence tests. |
| Smooth six-player large-map play | River Valley, 5,139 authoritative bodies, 120 Hz physics and 20 Hz snapshots, zero catch-up caps. Tick p95 2.319 ms; all-six snapshot p95 5.533 ms; about 1.05 MiB/s aggregate traffic on localhost. |
| Full session lifecycle | Socket tests cover countdown, capture/time limit, disconnect, return to lobby, map change and rematch; desktop checks cover the connected presentation flow. |
| Existing game and sandbox preserved | Workspace tests, both native/WASM builds and browser game/audio regressions pass. |

## Headless evidence

`bash tools/check-lan-headless.sh` exited 0 with display variables unset:
250 unit tests, four explicitly enabled socket scenarios, formatting, lint and
the six-client benchmark. See [the detailed report](LAN_HEADLESS_TESTS.md), which
includes raw output, timing distributions and hardware/measurement limitations.

## Desktop checks

Two distinct native release processes were exercised through their X11 windows.
The following were observed:

- Host discovery advertisements and direct IP joining; both team roster entries.
- Guest ready followed by host map change to River Valley clears readiness and
  updates the guest's map dimensions/object count.
- Host editor opens and returns to the network lobby; both players ready and
  host start produces countdown and separate Blue/Red gameplay views.
- Guest movement, aiming and firing produce replicated movement and destroyed
  wall geometry with the normal HUD, fog and effects.
- Host match menu leaves the guest clock running (09:29 to 09:25). Sound settings
  also leave the match running. Closing either returns to live play.
- Host return-to-lobby resets readiness for both players; guest leaves and rejoins
  through a discovery advertisement. Host exit surfaces a disconnect message.

This found a queued text-input bug: driving keys could later appear in the
player-name field. LAN entry and disconnect now discard queued characters. Fresh
release processes verified that the name remains `Commander` both after typing
before LAN entry and after driving/leaving a match. Native builds and lint were
rerun after this correction; the correction is excluded from WASM.

Window captures are retained under `target/screenshots/lan-*.png`, including
`lan-ready`, `lan-guest-match`, `lan-guest-moved`, `lan-menu`,
`lan-sound-settings`, `lan-back-lobby`, and both `lan-clean-name-*` captures.
Only the test game processes were closed afterward.

## Builds and existing browser behavior

- `cargo build --release --locked -p game -p sandbox`: [passed](validation/lan-native-2026-09-26.log).
- `bash tools/build-web.sh`: [both WASM binaries passed](validation/lan-web-2026-09-26.log).
- `bash tools/check-browser.sh game audio`: [both suites passed](validation/lan-browser-2026-09-26.log), including local map creation/persistence, countdown, capture/results/rematch, sandbox isolation and audio/settings behavior.
- Final formatting and clippy with existing project allowances: [passed](validation/lan-clippy-2026-09-26.log).

These checks use Linux, localhost sockets and two windows on the same computer.
They do not measure separate computers, Wi-Fi jitter/loss or lower-end hardware.
Discovery also produced a non-loopback interface advertisement on this machine.
The protocol uses TCP gameplay and UDP discovery; internet matchmaking, host
migration and joining an already running match are outside this LAN version.
