# Contributing

Start with the [current development summary](docs/HANDOFF.md) for accepted behavior and tuning. [Development history](docs/DEVELOPMENT_HISTORY.md) records the earlier experiments and rejected approaches. This project is a physics sandbox: preserve the accepted behavior while adding one concrete capability at a time.

## Run and check

Rust 2024, Cargo, and the `wasm32-unknown-unknown` target build both applications. Dependencies are pinned in `Cargo.lock`; use `--locked`. Linux native sound uses the ALSA runtime (`libasound.so.2`). Python 3 serves the browser files. No npm build step is needed.

```sh
cargo run --release --locked -p game      # local game
cargo run --release --locked -p sandbox   # development tools
bash tools/check.sh                 # format, workspace tests, clippy
bash tools/check.sh --web           # also rebuild both browser binaries
python3 tools/serve.py              # http://127.0.0.1:8000
```

Internal tests must be silent. Launch native game/sandbox tests with
`FRACTURE_SILENT=1` to mute playback without changing saved mixer settings.
For audio-output assertions, mute the application output stream instead.

For browser interaction checks, use Node 22+ and an isolated, muted Chrome profile:

```sh
google-chrome --headless=new --mute-audio --remote-debugging-port=9227 \
  --user-data-dir=/tmp/fracture-browser-test \
  --use-angle=swiftshader --enable-unsafe-swiftshader about:blank

bash tools/check-browser.sh editor comparison   # choose relevant suites
bash tools/check-browser.sh                     # all suites, sequentially
```

Start the server and browser in separate terminals. `GAME_URL` (default `/`), `SANDBOX_URL` (default `/sandbox.html`) and `CHROME_DEBUG_URL` override their addresses. Use an isolated profile because the suites exercise real local storage and import/export. Suites preserve the original stores in their cleanup handlers. Run them sequentially; they share that browser's storage. Screenshots go to `target/screenshots/`. Chromium executable names vary by installation.

For desktop LAN changes, run `bash tools/check-lan-headless.sh`. It includes real-socket tests and a six-client benchmark; see [LAN validation](docs/LAN_HEADLESS_TESTS.md).

For physics and frame-time changes, use the opt-in profilers and replay comparisons
in [the Deck performance guide](docs/PHYSICS_PERFORMANCE.md). Headless physics
timings alone do not establish gameplay FPS.

## Code map

`crates/sim` owns physics, assets and reproducible experiments. It has no renderer, window, browser or runtime network dependency. `crates/game` launches the game; `crates/sandbox` launches the development sandbox and provides their shared `fracture_client` library. That library owns input, drawing, editor/comparison state and platform persistence. Public simulation imports remain available from `sim::{World, Command, Shot, ...}` and `sim::experiment::{Experiment, Runner, ...}`.

| Change | Start here |
| --- | --- |
| Music, effects, mixing and volume settings | [audio.rs](crates/sandbox/src/audio.rs), [audio.js](web/audio.js), [AUDIO.md](docs/AUDIO.md) |
| LAN host, protocol, discovery and replication | [crates/net/src/](crates/net/src), [lan.rs](crates/sandbox/src/lan.rs), [LAN_MULTIPLAYER.md](docs/LAN_MULTIPLAYER.md) |
| Multiplayer simulation and presentation replicas | [multiplayer.rs](crates/sim/src/world/multiplayer.rs), [replica.rs](crates/sim/src/world/replica.rs) |
| Local lobby / match HUD / result screens | [game.rs](crates/sandbox/src/game.rs), [game/](crates/sandbox/src/game) |
| Match rules, timing and setup validation | [matchplay.rs](crates/sim/src/matchplay.rs) |
| World state and update order | [world.rs](crates/sim/src/world.rs) |
| Public commands, events and views | [world/api.rs](crates/sim/src/world/api.rs), [world/queries.rs](crates/sim/src/world/queries.rs) |
| World construction / test placement | [world/setup.rs](crates/sim/src/world/setup.rs), [world/vehicle.rs](crates/sim/src/world/vehicle.rs) |
| Built-in scenes / terrain recipe | [world/scenes.rs](crates/sim/src/world/scenes.rs), [world/scenes/terrain.rs](crates/sim/src/world/scenes/terrain.rs) |
| Bundled demo map / route checks | [quarry-crossing.json](web/levels/quarry-crossing.json), [demo_tests.rs](crates/sim/src/world/level/demo_tests.rs), [DEMO_MAP.md](docs/DEMO_MAP.md) |
| Portable maps / instance transforms | [sim/level.rs](crates/sim/src/level.rs) |
| Level loading / team flags and victory | [world/level.rs](crates/sim/src/world/level.rs) |
| Level editor / selection / clipboard / placement / inspector | [level_editor.rs](crates/sandbox/src/level_editor.rs), [level_editor/](crates/sandbox/src/level_editor) |
| Level persistence | [level_storage.rs](crates/sandbox/src/level_storage.rs), [web/levels.js](web/levels.js) |
| Authored object → cells and seams | [world/geometry.rs](crates/sim/src/world/geometry.rs) |
| Constrained map transforms and polygon unions | [level/authoring.rs](crates/sim/src/level/authoring.rs); polygon booleans use the pinned `i_overlay` crate |
| Shared terrain composition | [terrain_view.rs](crates/sandbox/src/terrain_view.rs) caches exposed edges and non-overlapping fills |
| Object schema, validation and mesh | [object.rs](crates/sim/src/object.rs), [object/meshing.rs](crates/sim/src/object/meshing.rs) |
| Object defaults and components | [object/library.rs](crates/sim/src/object/library.rs), [world/components.rs](crates/sim/src/world/components.rs) |
| Stress and fracture | [world/loads.rs](crates/sim/src/world/loads.rs), [structure.rs](crates/sim/src/structure.rs) |
| Rebuilding connected rigid bodies | [world/chunks.rs](crates/sim/src/world/chunks.rs) |
| Adaptive subdivision / laser deletion | [world/refinement.rs](crates/sim/src/world/refinement.rs) |
| Permanent deformation | [world/plasticity.rs](crates/sim/src/world/plasticity.rs) |
| Collision layers / weapon target policy | [world/collision.rs](crates/sim/src/world/collision.rs) |
| Ammunition defaults / impacts | [ammunition.rs](crates/sim/src/ammunition.rs), [world/weapons.rs](crates/sim/src/world/weapons.rs) |
| Terrain traction / bridge lifecycle | [terrain.rs](crates/sim/src/terrain.rs), [world/terrain.rs](crates/sim/src/world/terrain.rs) |
| Ground drag / fragment cleanup | [world/debris.rs](crates/sim/src/world/debris.rs) |
| Experiment schema / validation | [experiment.rs](crates/sim/src/experiment.rs) |
| Prepared studies / sweeps | [experiment/presets.rs](crates/sim/src/experiment/presets.rs), [experiment/sweep.rs](crates/sim/src/experiment/sweep.rs) |
| Running / recording experiments | [experiment/runner.rs](crates/sim/src/experiment/runner.rs), [experiment/recording.rs](crates/sim/src/experiment/recording.rs) |
| Window and screen transitions | [shared client](crates/sandbox/src/lib.rs), [game entry](crates/game/src/main.rs), [sandbox entry](crates/sandbox/src/main.rs) |
| Playable session and effects | [sandbox/lab.rs](crates/sandbox/src/lab.rs) |
| Scene shortcuts and track input | [sandbox/controls.rs](crates/sandbox/src/controls.rs) |
| Live world rendering / HUD | [sandbox/view.rs](crates/sandbox/src/view.rs), [sandbox/ui.rs](crates/sandbox/src/ui.rs) |
| Editor state and navigation | [sandbox/editor.rs](crates/sandbox/src/editor.rs) |
| Editor fields / panels / menus / canvas | [sandbox/editor/](crates/sandbox/src/editor) |
| Comparison state, playback and authoring | [sandbox/comparison.rs](crates/sandbox/src/comparison.rs), [sandbox/comparison/](crates/sandbox/src/comparison) |
| Native/browser storage boundary | [editor_storage.rs](crates/sandbox/src/editor_storage.rs), [comparison_storage.rs](crates/sandbox/src/comparison_storage.rs), [web/editor.js](web/editor.js), [web/experiments.js](web/experiments.js) |

The editor and comparison modules retain their state at the parent level. Their children implement focused methods on that state; `pub(super)` shares those methods within the screen, not with the simulation. This avoids introducing a general UI framework while making each area independently readable. Field parsing and rendering stay separate from canvas interaction and storage actions. Their existing keyboard/commit behavior is preserved.

## Physics rules to preserve

Read [DESTRUCTION.md](docs/DESTRUCTION.md) and [PLASTICITY.md](docs/PLASTICITY.md) before changing mechanics.

- Fixed simulation ticks are 1/120 s, each with four Rapier microsteps. `World::step` owns their order; rendering and slow motion do not change the timestep.
- A cell is material geometry. A connected chunk is **one rigid body**. The virtual elastic solve estimates stress; it does not displace intact visible/collision geometry. Do not restore physical spring joints between all cells.
- A crack needs both excessive stress and available fracture work. Re-equilibrate after a crack; do not break every seam from one stale stress result. A failed solve cannot initiate fracture.
- A split preserves material mass and inherits rigid motion. Never add separation kicks. Rapier contact impulses have already been applied; spend their bounded dissipated work without applying the impulse twice.
- Refinement transfers existing cracks, ground attachment, object identity, collision layer, shape revision, debris deadline and ancestral source IDs. It remaps pending loads/pulses without replenishing their energy.
- Terrain deck cells remain structural material but are not horizontal collision or weapon targets. Use the shared collision policy for new colliders and weapon queries; the A/B booths remain physical targets.
- Small disconnected chunks expire according to total chunk area, not individual triangle size. Compare removed area when checking different mesh resolutions.
- Preserve stable ordering in geometry insertion, connected components and the stress solver. Avoid randomized container iteration in simulation paths.

## Typical contributions

**Add an object:** extend `LibraryPreset` and its `asset()` recipe in `object/library.rs`. Prefer existing components when possible. For a new property, add validation/defaults to the asset/component schema, implement its response, and preserve it during refinement/rebuild. Add the relevant editor fields and material controls; comparisons need corresponding fields only when the property should be editable there. Exercise actual behavior with a small repeatable fixture, then use the library/editor browser checks. Built-in recipes must not overwrite users' saved copies.

**Add or tune ammunition:** edit `ammunition.rs` and `world/weapons.rs`; use `weapon_target` for collision selection. Keep cooldown/automatic fire inside the fixed-step simulation. Preserve the command/replay path and add a focused physical regression, then run `ammunition` and affected terrain checks. A timing change can invalidate saved visual expectations even when JSON remains readable.

**Add a scene or terrain rule:** construction belongs in `world/scenes*` and the `World::new` dispatch in `setup.rs`. The object yard is currently placed by `object_yard` in `world/components.rs`. Register a new scene’s button, label and key together in `sandbox/controls.rs`. `Scene::ALL` intentionally retains the three original benchmark fixtures; the playable menu uses `controls::SCENES`. Terrain placement and material recipes belong in `scenes/terrain.rs`; traction belongs in `sim/terrain.rs`; crossing/collapse behavior belongs in `world/terrain.rs`. Update rendering and the shared collision policy when needed.

**Add an editor/comparison control:** state/navigation stays in the parent screen, formatting and commit logic in `fields.rs`, controls in `panels.rs`, and drawing/interaction in `canvas.rs` or `screen.rs`/`view.rs`. Preserve undo, invalid-input handling, saved drafts, and comparison invalidation. Existing browser suites exercise those paths; add a scenario when the behavior is new.

**Add a level item:** read [LEVEL_EDITOR.md](docs/LEVEL_EDITOR.md). Add the recipe/validation in `sim/level.rs`, loading in `world/level.rs`, then placement, rendering and contextual controls in the map editor. Level copies embed assets. Shapes regenerate physical-size grains; do not silently modify library originals. Flags are nonphysical objectives: only a living enemy tank’s actual colliders can capture them. Preserve editor/runtime terrain ordering and bridge booth/deck separation. Run the `level` browser suite and relevant simulation tests.

## Persistence and compatibility

Object and experiment JSON remain version 1. Add optional fields with defaults when compatibility permits; keep validation and file/memory limits. `serde_json` uses `float_roundtrip` because exact f64 replay depends on it. Saved comparisons are **recipes plus visual checkpoints**, not Rapier snapshots. The runner owns fixed-tick execution and shared timing; recordings own immutable geometry revisions and frames.

Keep `ENGINE_REVISION` when a refactor preserves simulation/mesh behavior. Change it when replay behavior changes, and regenerate affected portable studies in `web/experiments/` using `examples/barrel_comparison.rs`. Preserve their authored parameters and accepted favorites. Never silently replace a user's saved recipe with new defaults.

## Verify a refactor

Capture before and after with the same platform, toolchain and build profile:

```sh
cargo run --release --locked -p sim --example regression_snapshot > /tmp/fracture-before.txt
# Make the refactor.
cargo run --release --locked -p sim --example regression_snapshot > /tmp/fracture-after.txt
diff -u /tmp/fracture-before.txt /tmp/fracture-after.txt
```

The example fingerprints every tick's observable physics state/events and all recorded frames across 36 scenarios/studies. It covers the library and all ammunition, terrain movement, bridge collapse and tank loss, plus the five prepared barrel studies. It is an additional regression signal, not a collision-proof equality proof or a cross-platform determinism guarantee. Do not regenerate an expected baseline merely to hide an unexplained difference.

Run `tools/check.sh`, build the affected native/browser targets, and run the browser suites for screens you changed. For a broad UI refactor, run the whole browser suite. Report what changed, what was verified, and any remaining limitation. Keep measured physics behavior separate from tuning targets.
