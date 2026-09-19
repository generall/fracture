# Development handoff

Current state: **2026-09-14**, after the level-editor interaction, geometry and scale improvements. The user accepted the latest bridge/booth behavior. Start here when continuing; [CONTRIBUTING.md](../CONTRIBUTING.md) maps code locations and workflows. [DEVELOPMENT_HISTORY.md](DEVELOPMENT_HISTORY.md) preserves the earlier decisions, experiments and rejected implementations.

## What we are building

A Rust tank-combat game with free movement, a top-down dark wireframe view and satisfying physical destruction. No abstract object health points, movement grid or textured scenery. The user is a software engineer who works mainly in Rust. Browser support works now; multiplayer is a later goal.

The project has progressed from a destruction sandbox to an editable object library, reproducible comparison range, multiple weapons, terrain/bridges and playable authored maps with team flags. It remains a sandbox rather than a complete game: no AI enemies, sound, detachable machine assemblies or multiplayer yet.

## Working features

- **Quarry Crossing demo:** M → File → Load Demo / Quarry. An editable 48 × 30 m, 39-item map with a central bridge, sandy north bypass, southern road, paired bases, rock/wall cover, fences and barrel yards. Canonical recipe: `web/levels/quarry-crossing.json`, also downloadable. Both teams can capture via the bridge, and via either bypass after HE destroys the bridge. See [DEMO_MAP.md](DEMO_MAP.md).

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
| HE | 6 kg at 220 m/s; 300 kJ blast, radius 1.7 m; 0.45 s reload. |
| AP | 8 kg at 350 m/s; concentrated 800 kJ blast, radius 0.65 m; 0.45 s reload. Stronger local damage, no penetration continuation yet. |
| MG | 24 g at 850 m/s; 12 rounds/s; no blast. Dents barrels, bends fences, ignites explosive barrels and breaks thin panels. |
| Laser | 6 cutting pulses/s; first subdivide, then remove one leaf of area ≤0.0125 m²; no recoil/impulse. Beam aim refreshes between pulses. |

All weapons share reload state; switching cannot bypass it. Automatic fire requests occur at fixed simulation ticks. Saved recipes retain authored mass/speed rather than silently adopting new defaults. See [AMMUNITION.md](AMMUNITION.md) and [OBJECT_LIBRARY.md](OBJECT_LIBRARY.md).

## Latest accepted terrain and bridge rules

Press **9 / TERRAIN**. Road/bridge multiply the default speed limit by **1.45**, engine force by **1.3**, grip/braking by **1.4**, and lateral damping by **1.35**. Sand uses **0.48 / 0.6 / 0.65 / 0.8**. Ordinary ground is unchanged. Four contact patches sample the surfaces.

Water blocks the complete tank hull/tracks with CCD, including diagonal approaches. Openings in the water barriers correspond to intact bridge decks. The bridge has a booth **A/B on each bank**, beside its entrance. Booth contact resistance is **1000**, projectile resistance **1**. It survives sustained pushing, 2–3 m/s direct bumps and 4.6 m/s glancing contacts in the regression fixtures. A hard direct ram can still destroy it; damage is geometry/load dependent, not a speed or hit-count rule.

**The deck cannot be targeted.** HE/AP/MG sweeps, muzzle clearance, laser traces and blast rays all skip it, from either bank or while standing on it. Shots continue to objects beyond the bridge. Only the booths receive direct weapon loads. Their stress can propagate through the connected structural mesh.

The span collapses when no live material component connects both bank fixtures. A small chip does not automatically collapse it. Close the crossing immediately, retire the span's material/colliders/loads, and animate its geometry sinking for 1.4 simulated seconds. A tank whose physical footprint overlaps newly exposed water is disabled and sinks too; a tank entirely on a bank survives. **R** restores the scene. Retain the disabled tank's pose record so rendering and diagnostics stay valid. See [TERRAIN.md](TERRAIN.md).

This is a single-span 2.5D rule, not vertical/fluid simulation. No sag, weight capacity, independent falling spans, terrain editor or local-hole tank hazards yet. Loose objects still use the horizontal floor.

## Physics and persistence invariants

Simulation uses meters, kilograms and seconds; horizontal gravity is zero. **120 Hz, four microsteps**; each Rapier step uses one TGS step and 12 PGS sweeps. Keep the exact update order in `world.rs`. Virtual elastic displacement never moves an intact brittle chunk's geometry. A failed structural solve cannot initiate cracks.

Fracture requires stress overload and bounded work for `Gc × face width × depth × cohesion`; re-equilibrate after each crack, at most eight rounds per pass. Split chunks inherit the parent rigid velocity field and preserve mass/momentum. Do not add separation impulses or reapply Rapier's contact impulses. Preserve load/pulse energy and material points through subdivision.

Refinement keeps existing cracks, support attachments, object identity, collision layer, shape revision, ancestral source IDs and debris deadlines. Maximum eight refinement generations and **8,192 retained records**, including retired parents. Tiny **disconnected chunks** with total area strictly below **0.075 m²** fade after **3.0–3.9 s**. Large chunks stay until further damage; small cells inside them do not expire individually.

Plastic deformation uses rim-edge constraints, **96 projection sweeps**, bounded work/rate/motion and inversion checks; maximum projected movement 15 mm per step. The user rejected extra contact-corner/convexity guards because they worsened flattening. Do not reintroduce that approach casually. [DESTRUCTION.md](DESTRUCTION.md) and [PLASTICITY.md](PLASTICITY.md) explain the models.

Object and experiment JSON remain **v1**. `serde_json` is pinned with **float_roundtrip** for f64 replay. Engine revision remains **`experiment-terrain-bridges-13`**: existing object-comparison replays preserve their behavior. The playable speed limit and authored bridge grid have changed; saved trials still apply their explicit speed. Experiments store full recipes and immutable visual checkpoints, not Rapier state snapshots. Same-build replay is tested; cross-platform or cross-revision bit identity is not promised.

Storage: browser draft `fracture.object.v1`, comparisons `fracture.experiments.v1`, object library `fracture.objects.v1`; native object/experiment files under `objects/`, and level files under `levels/`. Preserve unreadable/invalid saved data and report the error. See [EDITOR.md](EDITOR.md) and [EXPERIMENTS.md](EXPERIMENTS.md) for formats and limits.

## Contribution refactor

Public `sim` imports, JSON fields, defaults, UI layout and controls remain compatible. The broad modules have been separated by responsibility:

- `world.rs` owns state and tick order; `world/api.rs` owns commands/events/views; `setup.rs`, `queries.rs`, `debris.rs` and `geometry.rs` handle construction, inspection, cleanup and material geometry.
- `world/scenes/terrain.rs` owns the terrain/bridge recipe; `world/terrain.rs` owns crossing/collapse. `world/collision.rs` centralizes collision layers and weapon target filtering.
- `experiment.rs` owns recipe/schema validation. `experiment/{presets,sweep,runner,recording}.rs` separate prepared studies, parameter changes, execution and immutable frames. Mechanics/experiment tests live in companion modules.
- `sandbox/main.rs` coordinates the window and screen transitions; `lab.rs` owns the playable session/effects; `controls.rs` is the shared scene-button/shortcut catalog and track input.
- Editor and comparison child modules separate fields, panels, canvas/record rendering, menus/library and browser diagnostics. `ui::wrapped` is a shared text helper.

Use [CONTRIBUTING.md](../CONTRIBUTING.md) for the detailed file map and recipes for adding an object, weapon, terrain rule or editor control. `bash tools/check.sh [--web]` runs standard Rust checks; `bash tools/check-browser.sh [suite ...]` runs browser suites sequentially. The `regression_snapshot` example provides before/after behavior fingerprints across **36 scenarios and prepared studies**.

Map geometry helpers and unions live in `sim/level/authoring.rs` (pinned `i_overlay`), with a robust fallback in `object/meshing.rs` for grain clipping. Cached terrain composition lives in `sandbox/terrain_view.rs`; multi-selection/clipboard actions live in `level_editor/selection.rs`. The map schema/runtime live in `sim/level.rs`, `world/level.rs`, and the sandbox `level_editor` modules; `level_storage.rs` / `web/levels.js` own file and draft persistence.

## Validation and next work

The current build passes **120 simulation tests and 4 sandbox tests**, formatting and clippy with warnings denied, plus native and WebAssembly release builds. Five relevant browser suites pass: level, level-improvements, demo, terrain and editor. They cover selection/clipboard, actual unions, border and dimension clamping, 190 m bridges, circular barrels, unbreakable walls, both team victories, terrain and bridge targeting, persistence and high-DPI rendering. New native tests check mesh cost and area, real merged rigid bodies, booth targetability at large scale, and actual terminal speed and braking on each dry terrain. Existing destruction, plasticity and replay tests remain green. All 36 pre-change behavior fingerprints match when the new authoring-only `circular` field is omitted from the comparison JSON digest. Existing experiment physics and saved trial parameters remain unchanged.


Build/run: `cargo run --release --locked -p sandbox`, or `bash tools/build-web.sh` followed by `python3 tools/serve.py`. Controls: WASD, Q/Z left track, E/C right track; mouse aim/fire; 5–8 ammo/B cycle; 1–4 reference scenes/object yard; 9 terrain; O object editor; M level editor; H manual; R rebuild; Space pause; N step; T slow; Tab stress; wheel/right-drag/Home camera.

Potential next work, to choose with the user: profile topology rebuild spikes on actual browser hardware; continue object/mesh calibration in the experiment range; saved scene experiments and multi-tank play; better crack geometry; machine parts and functional failures; sound/gameplay; later authoritative multiplayer. Preserve accepted driving/deformation feel. Current limits include triangle-constrained cracks, quasistatic stress, mesh/timestep-dependent fracture, no continuum tearing/fatigue/stress waves, no AP penetration, and no general vertical stacking/fluid dynamics.
