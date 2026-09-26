# Development handoff

**Debris budget (2026-09-26):** Levels now target 8,192 active pieces by retiring
small, distant disposable chunks at 4 Hz. All tank sight ranges, flag lookouts
and owned radar coverage receive an 8 m protection margin. Whole chunks retire
smallest-first, then farthest-first; new wrecks and intact structures remain.
The target is soft if no safe debris is available. Stable retired IDs and the
separate subdivision budget are unchanged.

**Wrecks on large maps (2026-09-26):** Removed the obsolete 8,192-total-record
guard from tank wreck creation. Saved match maps can exceed that count before
play begins, which suppressed hull/track remains while the separate turret
animation still played. Wrecks retain their 2.5-second lifetime and retire before
respawn; a regression test covers levels starting above the old limit.

**Laptop controls (2026-09-26):** Q/E or J/K rotate the turret left/right,
Space (also left-click/F) fires, and G (also right-click) activates the collected
special weapon. Keyboard aim holds its direction while driving until the mouse
moves again. WASD drives; the old Q/Z and E/C track overrides are removed.
Esc opens the match menu; P pauses the sandbox. Comparison playback keeps Space.

**Apple Silicon and input carets (2026-09-26):** Both applications cross-build for
`aarch64-apple-darwin` (macOS 11+) using `tools/build-macos-arm64.sh`; see
[macOS packaging](MACOS_BUILD.md). The artifact is
`target/dist/fracture-macos-arm64.zip`, with separate game/sandbox executables.
Focused LAN, object-editor, comparison and map-editor fields now display a cyan
caret, including empty and horizontally scrolled values. Browser captures and
native LAN focus checks verified the display. macOS runtime testing remains open.

**LAN multiplayer (2026-09-26):** Desktop host/IP/discovery joining, six-player team/ready lobby, host-owned physics, binary/LZ4 visibility-filtered deltas and interpolated client replicas are implemented. Headless validation passed first: 250 unit tests, four real-socket scenarios and a six-client River Valley benchmark. Native/WASM builds, browser game/audio regressions and desktop lobby/match checks also passed. See [run/join instructions](LAN_MULTIPLAYER.md), [headless measurements](LAN_HEADLESS_TESTS.md) and [acceptance audit](LAN_ACCEPTANCE.md). Physical multi-computer LAN/Wi-Fi performance has not been measured.

**Earlier saves:** The completed local-game archive and the initial multiplayer work-in-progress archive are preserved. The latter predates transport and UI implementation; its limitations describe that archive only. See [checkpoint notes](checkpoints/2026-09-26-current-progress.md).

**Decorative destruction sounds (2026-09-26):** Shrubs, crates and timber now have quiet material sounds triggered once by `DecorationDestroyed`, covering tank contact, weapon sweeps and blasts. They share normal effects volume, distance attenuation and coalescing. Decorations remain nonphysical; their visual burst lifecycle is unchanged.

**Terrain feedback (2026-09-26):** Movement audio blends quiet road, sand, grass and bridge textures into the existing engine/treads; elevation adds light grit. Cliff terrain now deflects loose physical objects as well as tanks through a separate collision group, while sight and all weapons still pass over it. Water retains its tank-only barrier. See [AUDIO.md](AUDIO.md) and [TERRAIN.md](TERRAIN.md).

**Small-object sight (2026-09-26):** Player and AI sight exclude props and detached chunks below 0.5 m² before bounds/silhouettes/ray casts. Classification uses connected chunks (whole authored objects for indestructible triangle bodies), preserving intact finely meshed cover. Collision and weapons retain their existing geometry. See [TANK_COMBAT.md](TANK_COMBAT.md).

**Sound design (2026-09-26):** Both binaries now include original sampled-orchestra 88 BPM military / 132 BPM battle music, visibility-driven crossfades, weapon/vehicle/destruction feedback, and a dedicated F7 sound settings screen (four saved sliders), and F8/F9/F10 shortcuts. Pickup/reload cues are quieter; actual barrel buckling and metal-fence yielding emit dedicated crunch sounds. Desktop screech fixes vendor `quad-snd` 0.2.8 through a Cargo patch: correct loop wrapping, smooth gain changes/stops, bounded transient overlap and peak limiting. Harsh effects are filtered offline. Run `cargo test -p quad-snd --lib --locked` alongside workspace tests. Audio is presentation-only. See [AUDIO.md](AUDIO.md).

**Latest addition (2026-09-26):** The default app now opens a title screen and local lobby, with map creation/import/selection, Blue/Red selection, match duration and AI behavior. A dedicated match HUD, countdown, pause menu, capture/time-limit results and rematches replace sandbox controls during play. See [GAME_MODE.md](GAME_MODE.md). `sim::matchplay` owns session rules; `sandbox/game` owns presentation. Game and sandbox now have separate binaries: `cargo run --release --locked -p game` / `-p sandbox`, and browser `/` / `/sandbox.html` load `game.wasm` / `sandbox.wasm`. Shared client code lives in `sandbox/src/lib.rs`; each executable selects its own application.

Current state: **2026-09-26**, after desktop LAN multiplayer, local match flow, separate applications, sound design, visibility filtering and terrain/decorative feedback. The user requested a saved checkpoint; see [the checkpoint notes](checkpoints/2026-09-26.md). Start here when continuing; [CONTRIBUTING.md](../CONTRIBUTING.md) maps code locations and workflows. [DEVELOPMENT_HISTORY.md](DEVELOPMENT_HISTORY.md) preserves the earlier decisions, experiments and rejected implementations.

## What we are building

A Rust tank-combat game with free movement, a top-down dark wireframe view and satisfying physical destruction. No abstract object health points, movement grid or textured scenery. The user is a software engineer who works mainly in Rust. Browser support and desktop LAN multiplayer work now.

The project now has a local game with a lobby, map selection/creation, an AI opponent, match rules, results and rematches. A separate sandbox retains the editors and material experiments. Weapons, terrain/bridges, vision, music and effects are implemented. Desktop LAN play supports up to six humans; detachable machine assemblies are not implemented.

## Working features

- **Quarry Crossing demo:** M → File → Load Demo / Quarry. An editable 48 × 30 m, 31-object map with a half-metre terrain mask with a central bridge, sandy north bypass, southern road, paired bases, rock/wall cover, fences and barrel yards. Canonical recipe: `web/levels/quarry-crossing.json`, also downloadable. Both teams can capture via the bridge, and via either bypass after HE destroys the bridge. See [DEMO_MAP.md](DEMO_MAP.md).

- **Level editor (M):** Shift-click and box-select groups; Ctrl A/C/V/X/D/J select/copy/paste/cut/duplicate/merge. Group movement and dimensions clamp at map borders. Auto Join combines matching rocks/walls into actual polygon unions with one regenerated fracture mesh; compatible axis-aligned walls snap together. Alt temporarily disables joining. Barrels stay circular during authoring. Large objects coarsen automatically, and bridges support spans up to 190 m with a bounded grid and fixed-size booths. Special tools include explicit unbreakable walls. Terrain overlaps render seamlessly, without sand/water/bridge titles. Place independent library copies, reshape/rotate polygons and add terrain and team spawns/flags. Map settings, snap, undo/redo, automatic draft saving and JSON import/export are implemented. Playtesting clones the recipe; R restarts and M returns. Test either Blue or Red with one controllable tank. A living enemy tank touching a flag wins immediately; flags ignore all weapons/physics damage. See [LEVEL_EDITOR.md](LEVEL_EDITOR.md) for controls, limits and code ownership. Local draft key is `fracture.level.v1`; native maps use `levels/`. Object JSON adds an optional `circular` authoring constraint; legacy named barrels remain recognized. Accepted material/deformation presets remain unchanged.

| Area | Current behavior |
| --- | --- |
| Tank | 6.5 m/s ground top speed (9.425 road, 3.12 sand); saved trials keep their authored speed. 4,500 kg compound hull/tracks; four traction patches; independent left/right tracks; strong braking and rotational scrub; turret aims independently. |
| Destruction | Each connected chunk is one rigid body. A separate elastic graph estimates stress; overloaded seams break only with available fracture work. |
| Local detail | Impacts refine material locally, including large detached triangles. Repeated hits can make them small enough to clear. |
| Permanent deformation | Barrels and metal fences deform permanently using a separate plastic-frame projection; intact detail does not make them rubbery. |
| Object editor | Outline/coordinate editing, stone grain points, staggered bricks, mesh detail, edge pins, ground anchors, colors and material/component controls; undo, drafts, import/export. |
| Library | Ordinary/explosive barrels, wall, irregular grounded rock and metal fence; named editable copies. |
| Comparisons | Four general variants, ten barrel variants or six shared-test variants; runtime impact settings, linked playback, timeline, favorite, saved recipe and visual checkpoint. |
| Weapons | HE, AP, MG and laser. Shots are swept and consumed at impact. Laser subdivides then removes small cells without force. |
| Terrain | Default ground, faster/grippier road, slower sand, impassable water, bridge with two destructible booths. |
| Rendering | Dark background, high-contrast bold text, lines/shapes, tracks, tracers, sparks, explosions, stress overlay and smooth cursor-anchored zoom. |

## Accepted tuning — preserve these defaults

**Barrel:** green, radius 0.65 m, density 140 kg/m³, depth 0.4 m, friction **0.09**. Permanent deformation: yield **500 Pa**, flow **2/s**, hardening **0**, strain limit **3** (accepted case 6). Bounce: base restitution **0.04**, soft-impact restitution **0.62**, transition speed **16 m/s** (accepted wall-release case 2). This retains free movement while releasing from a backstop. The explosive barrel has its own plastic/bounce settings; do not copy the ordinary barrel over it.

**Wall:** amber; density 2,400 kg/m³, depth 0.55 m, friction 1.3; ordinary masonry seam strengths **88/400/64 kPa** tension/compression/shear; Gc **55 J/m²**, cohesion **4**, **0.7 × 0.6 m** bricks, one edge-2 pin, contact resistance 1.5 and no contact refinement. The user explicitly accepted this after rejecting a tougher wall. Do not restore the earlier 16× strength/two-pin profile.

**Rock:** violet, irregular outline; density 2,900 kg/m³, depth 0.65 m, friction 1.1; same 88/400/64 kPa seams, E **15 GPa**, Gc **55 J/m²**, cohesion **18**, **0.9 × 0.75 m** stone grains. Central buried-grain ground attachment; contact resistance **1000**, no contact refinement. Shells chip it, tanks cannot push the core. User accepted it.

**Object spin:** floor angular deceleration is **10 rad/s² at friction 0.32**, four times the original setting. Apply drag once per dynamic chunk. The tank has its own track traction model.

| Ammunition | Accepted settings |
| --- | --- |
| HE | 6 kg at 220 m/s; 300 kJ blast, radius 1.7 m; 0.9 s reload. |
| AP | 8 kg at 350 m/s; concentrated 800 kJ blast, radius 0.65 m; 0.9 s reload. Stronger local damage, no penetration continuation yet. |
| Rocket | Single-use pickup, right click; 16 kg at 12 m/s, 5.5 m blast and internal material shock. |
| Laser | Single-use pickup, right click; unstoppable 3 s beam, 0.6 m wide, 10 cutting pulses/s, zero spread. |
| Plasma cannon | Single-use pickup, right click; 1 s charge while aiming, then one pulse with the full laser budget (30 pulses / up to 240 cutting operations). Special → Plasma in the level editor. |

HE/AP share main-gun reload state; switching starts a full 0.9 s reload. Specials use a separate one-item inventory. Saved recipes retain authored mass/speed rather than silently adopting new defaults. See [AMMUNITION.md](AMMUNITION.md) and [OBJECT_LIBRARY.md](OBJECT_LIBRARY.md).

## Latest accepted terrain and bridge rules

Press **9 / TERRAIN**. Road/bridge multiply the default speed limit by **1.45**, engine force by **1.3**, grip/braking by **1.4**, and lateral damping by **1.35**. Sand uses **0.48 / 0.6 / 0.65 / 0.8**. Ordinary ground is unchanged. Four contact patches sample the surfaces.

Water blocks the complete tank hull/tracks with CCD, including diagonal approaches. Openings in the water barriers correspond to intact bridge decks. The bridge has a booth **A/B on each bank**, beside its entrance. Booth contact resistance is **1000**, projectile resistance **1**. It survives sustained pushing, 2–3 m/s direct bumps and 4.6 m/s glancing contacts in the regression fixtures. A hard direct ram can still destroy it; damage is geometry/load dependent, not a speed or hit-count rule.

**The deck cannot be targeted.** HE/AP/MG sweeps, muzzle clearance, laser traces and blast rays all skip it, from either bank or while standing on it. Shots continue to objects beyond the bridge. Only the booths receive direct weapon loads. Their stress can propagate through the connected structural mesh.

The span collapses when no live material component connects both bank fixtures. A small chip does not automatically collapse it. Close the crossing immediately, retire the span's material/colliders/loads, and animate its geometry sinking for 1.4 simulated seconds. A tank whose physical footprint overlaps newly exposed water is disabled and sinks too; a tank entirely on a bank survives. **R** restores the scene. Retain the disabled tank's pose record so rendering and diagnostics stay valid. See [TERRAIN.md](TERRAIN.md).

This is a single-span 2.5D rule, not vertical/fluid simulation. No sag, weight capacity, independent falling spans, terrain editor or local-hole tank hazards yet. Loose objects still use the horizontal floor.

## Physics and persistence invariants

Simulation uses meters, kilograms and seconds; horizontal gravity is zero. **120 Hz, four microsteps**; each Rapier step uses one TGS step and 12 PGS sweeps. Keep the exact update order in `world.rs`. Virtual elastic displacement never moves an intact brittle chunk's geometry. A failed structural solve cannot initiate cracks.

Fracture requires stress overload and bounded work for `Gc × face width × depth × cohesion`; re-equilibrate after each crack, at most eight rounds per pass. Split chunks inherit the parent rigid velocity field and preserve mass/momentum. Do not add separation impulses or reapply Rapier's contact impulses. Preserve load/pulse energy and material points through subdivision.

Refinement keeps existing cracks, support attachments, object identity, collision layer, shape revision, ancestral source IDs and debris deadlines. Maximum eight refinement generations. Playable levels allow **8,192 additional runtime subdivision records**, including retired children, independently of starting map geometry; material experiments retain the original **8,192 total-record** cap. Tiny **disconnected chunks** with total area strictly below **0.075 m²** fade after **3.0–3.9 s**. Large chunks stay until further damage; small cells inside them do not expire individually.

Plastic deformation uses rim-edge constraints, **96 projection sweeps**, bounded work/rate/motion and inversion checks; maximum projected movement 15 mm per step. The user rejected extra contact-corner/convexity guards because they worsened flattening. Do not reintroduce that approach casually. [DESTRUCTION.md](DESTRUCTION.md) and [PLASTICITY.md](PLASTICITY.md) explain the models.

Object and experiment JSON remain **v1**. `serde_json` is pinned with **float_roundtrip** for f64 replay. Engine revision remains **`experiment-terrain-bridges-13`**: existing object-comparison replays preserve their behavior. The playable speed limit and authored bridge grid have changed; saved trials still apply their explicit speed. Experiments store full recipes and immutable visual checkpoints, not Rapier state snapshots. Same-build replay is tested; cross-platform or cross-revision bit identity is not promised.

Storage: browser draft `fracture.object.v1`, comparisons `fracture.experiments.v1`, object library `fracture.objects.v1`; native object/experiment files under `objects/`, and level files under `levels/`. Preserve unreadable/invalid saved data and report the error. See [EDITOR.md](EDITOR.md) and [EXPERIMENTS.md](EXPERIMENTS.md) for formats and limits.

## Contribution refactor

Public `sim` imports, JSON fields, defaults, UI layout and controls remain compatible. The broad modules have been separated by responsibility:

- `world.rs` owns state and tick order; `world/api.rs` owns commands/events/views; `setup.rs`, `queries.rs`, `debris.rs` and `geometry.rs` handle construction, inspection, cleanup and material geometry.
- `world/scenes/terrain.rs` owns the terrain/bridge recipe; `world/terrain.rs` owns crossing/collapse. `world/collision.rs` centralizes collision layers and weapon target filtering.
- `experiment.rs` owns recipe/schema validation. `experiment/{presets,sweep,runner,recording}.rs` separate prepared studies, parameter changes, execution and immutable frames. Mechanics/experiment tests live in companion modules.
- `sandbox/lib.rs` coordinates the window and screen transitions; `lab.rs` owns the playable session/effects; `controls.rs` is the shared scene-button/shortcut catalog and track input.
- Editor and comparison child modules separate fields, panels, canvas/record rendering, menus/library and browser diagnostics. `ui::wrapped` is a shared text helper.

Use [CONTRIBUTING.md](../CONTRIBUTING.md) for the detailed file map and recipes for adding an object, weapon, terrain rule or editor control. `bash tools/check.sh [--web]` runs standard Rust checks; `bash tools/check-browser.sh [suite ...]` runs browser suites sequentially. The `regression_snapshot` example provides before/after behavior fingerprints across **36 scenarios and prepared studies**.

Map geometry helpers and unions live in `sim/level/authoring.rs` (pinned `i_overlay`), with a robust fallback in `object/meshing.rs` for grain clipping. Cached terrain composition lives in `sandbox/terrain_view.rs`; multi-selection/clipboard actions live in `level_editor/selection.rs`. The map schema/runtime live in `sim/level.rs`, `world/level.rs`, and the sandbox `level_editor` modules; `level_storage.rs` / `web/levels.js` own file and draft persistence.

## Validation and next work

Latest verification: **197 simulation + 36 client tests passed**, with one pre-existing ignored test. Formatting passed. Clippy passed with the existing `chunks_exact_to_as_chunks`, `items_after_test_module` and `approx_constant` allowances. Native and WebAssembly release builds for both applications succeeded. The audio browser suite passed with all **32 assets**, real output/mute checks, music visibility transitions, road-to-sand movement audio, actual shrub/crate/timber destruction, settings persistence and input isolation. The vendored mixer has five passing regression tests, run separately via `cargo test -p quad-snd --lib --locked`.

Earlier editor/game/weapon suite results are historical, not fresh validation of this checkpoint. The old general browser smoke test has stale scene-number expectations, and the plasticity smoke test assumes the older fence preset; update those fixtures before using them as a blanket gate.


Build/run: `cargo run --release --locked -p game` for the local game; `cargo run --release --locked -p sandbox` for development tools. Browser: `bash tools/build-web.sh`, then `python3 tools/serve.py`; `/` runs the game and `/sandbox.html` runs the sandbox. F7 opens sound settings; F8 mutes. See [GAME_MODE.md](GAME_MODE.md), [AUDIO.md](AUDIO.md) and [SANDBOX.md](SANDBOX.md) for mode-specific controls.

Potential next work, to choose with the user: profile topology rebuild spikes on actual browser hardware; continue object/mesh calibration in the experiment range; measure multiplayer on separate computers and Wi-Fi; better crack geometry; machine parts and functional failures. Preserve accepted driving/deformation feel. Current limits include triangle-constrained cracks, quasistatic stress, mesh/timestep-dependent fracture, no continuum tearing/fatigue/stress waves, no AP penetration, and no general vertical stacking/fluid dynamics.
