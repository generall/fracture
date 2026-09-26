**Development history — archived before the contribution refactor**

This is a chronological record, including superseded tuning and earlier limitations. Start with [HANDOFF.md](HANDOFF.md) for the current accepted behavior and [CONTRIBUTING.md](../CONTRIBUTING.md) for the code map.

**Development handoff — 2026-09-13**

**Latest: stronger bridge booths; deck is untargetable**

User feedback: bridge checkpoints collapsed too easily when rammed, and only the side booths should be projectile targets. The bridge recipe now sets **contact resistance 1000** instead of inheriting the wall's 1.5. Projectile resistance remains 1; ordinary wall/rock/barrel profiles are unchanged. This effective contact multiplier applies to stress failure and fracture work, without changing impulse or introducing an HP counter. Measured on both banks: 10 seconds of pushing, direct 2–3 m/s bumps and 4.6 m/s glancing contact leave the bridge intact. A direct 4.6 m/s ram can still break it. Impact angle, contact locations and existing cracks matter; this is not a simple speed threshold.

All shell sweeps (including muzzle clearance), laser traces and blast rays now skip cells marked `bridge_deck`, regardless of where the tank fires from. Deck cells remain in the structural mesh, so booth damage can break the load path and trigger collapse. Only the booths are direct weapon targets; projectiles continue to objects across the river. Removed the obsolete per-shell `above_bridge` state and tank-specific deck-filter helpers. The scene hint and A/B labels now identify booths. Sinking/crossing rules are unchanged.

Added regressions for both-bank pushing/bumps/glances, all four weapons reaching a target beyond the deck, and blasts over the deck creating no material loads. Existing real shell/ram collapse and occupant-loss tests still pass. Terrain replay now aims at a booth, consistent with the new target rule. Extended browser terrain smoke to fire all four weapons over the deck before crossing and destroying a booth from the bridge. Engine revision remains `experiment-terrain-bridges-13`; this terrain-only recipe change does not affect the serialized object-comparison fixtures. See [TERRAIN.md](TERRAIN.md) for current rules.

**Validation: 98 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native and WASM release builds, and the extended browser terrain smoke (all weapons clear the deck; shooting a booth collapses the bridge and sinks its occupant).

**Previous: terrain and destructible bridges (deck targeting and booth resistance superseded above)**

Added **9 / TERRAIN**, a river-crossing scene with a road through its center and sand on opposite corners. Existing scenes and comparisons retain default ground. Four track patches sample local terrain: road/bridge speed ×1.45, drive force ×1.3, grip/braking ×1.4, lateral damping ×1.35; sand ×0.48 / ×0.6 / ×0.65 / ×0.8 respectively. Ground preserves the previous force calculations exactly. No accepted object or ammunition values changed.

The bridge is one concave destructible masonry mesh: a walkable deck with two exposed side piers (A/B), pinned to bank fixtures. Both supports can be shot or rammed; bank-fired shells can hit deck edges and laser cuts use the existing adaptive grid. A bridge survives local chips but collapses when no live material chunk connects both fixtures, including two separately anchored halves after a complete cut. This is physical connectivity, not an HP threshold. The check uses chunk identity after fracture/rebuild and runs on topology changes.

Collision layers separate normal objects (group 1), tank (2), and water barriers (3). Deck cells have no horizontal collision response, but remain weapon/stress targets. Deck identity and collision groups survive refinement and chunk rebuilds. Water rectangles are cut around intact deck openings; full hull/track colliders enforce water boundaries with CCD. Terrain colliders have no piece ID and are excluded from all weapon queries. Shells retain the bridge layer they were launched above so a turret on a bridge can hit its exposed supports without its own deck blocking the muzzle.

Support loss immediately closes the crossing. Capture the surviving span geometry for a 1.4-second sinking animation, retire its material/loads/colliders, and keep the bank fixtures. Check the tank's actual footprint against the newly exposed water before the contact solver can push it onto shore. A caught tank is disabled, sinks visually, and cannot drive or fire; it keeps its body/pose record for safe rendering and diagnostics. A tank wholly on a bank survives. **R** restores both bridge and tank. Public terrain/bridge views and tank lifecycle methods are in `terrain.rs` and `world/terrain.rs`; rendering is in `view.rs`. Read-only browser diagnostics add `lab_terrain`, `lab_bridges`, `lab_tank_alive`.

Scope: first authored, axis-aligned terrain scene, single-span collapse using a 2.5D layer rule. No vertical-fluid solver, sag/load capacity, individual falling spans, local-hole tank hazards, terrain editor or terrain experiment-file format yet. Loose objects still use the horizontal floor. See [TERRAIN.md](TERRAIN.md).

Engine revision **`experiment-terrain-bridges-13`**. Portable barrel recipes regenerated with their existing properties/favorites. Eight new terrain tests cover baseline driving, surfaces, full-footprint water, bidirectional crossing, real shell/ram failure, local laser cuts and collision inheritance, severed but separately anchored halves, closed crossing, tank loss/controls, ballistic transparency and deterministic commands. Browser terrain smoke verifies live crossing, support destruction while on the deck, sinking and rebuild. **Validation: 95 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native/WASM release builds, existing gameplay browser checks (including high DPI and smooth zoom), and the new terrain browser checks. Inspected `target/screenshots/terrain-ready.png`, `terrain-crossing.png`, `terrain-collapse.png` and `terrain-sunk.png`. Updated the gameplay smoke test’s scene-button coordinates for the five-entry header.

**Previous: AP / MG buff and slower laser**

User feedback: AP felt like a worse HE, MG could not damage anything useful, and the laser cut too quickly. Kept all accepted object profiles and HE behavior. AP retains its 8 kg / 350 m/s projectile and **0.65 m blast radius**, but the concentrated charge now supplies **100 kJ/kg (800 kJ default)** instead of 5 kJ/kg. Lower increases did not improve the wall materially. The final centered shot produces 17 wall chunks (HE 14), with **16 fractures within 0.65 m of impact vs HE 12**; rock local fractures are **3 vs 1**, ordinary plate **16 vs 4**. The ordinary barrel deforms more under AP as well. This is concentrated blast tuning, not a new penetration solver.

MG defaults are now **24 g / 850 m/s = 8.67 kJ**, six times the prior 8 g / 600 m/s round, still **12 rounds/s with no bullet blast**. Two-second trials visibly dent the ordinary barrel (~3% peak strain), bend the fence (~1.7%), ignite an explosive barrel and fragment a thin panel. The wall develops three cracked seams but stays in one chunk; the rock remains uncracked. Regression coverage now checks actual light library starters, not just an unusually thin panel. Saved MG recipes retain their own mass/speed; cycle away/back to MG for the new default.

Laser cutting slows from **30 to 6 pulses/s**, with the same 0.0125 m² deletion threshold. Beam tracing refreshes each held-fire tick even between cutting pulses, while the shared cooldown gates all material changes. Releasing stops work immediately; the cosmetic beam expires after 60 ms. A two-second wall test removes about **0.0164 m² (4 tiles)** instead of 0.1116 m² (32 tiles). Tests cover the slower cadence, responsive aim between pulses, release, force-free cutting and a longer-duration completed cut. Laser limit notices now survive the Fired event ordering.

Engine revision: **`experiment-ammunition-balance-12`**. All five portable HE/barrel comparisons are regenerated with their prior materials and favorites. `examples/ammunition.rs` now includes the ordinary plate, explosive barrel and fence, and reports fractures near the impact. New tests compare AP’s local damage against HE and verify MG deformation/ignition. Browser ammunition checks exercise actual starter dents/ignition and bound the slower laser’s progress.

Verification: **87 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native/WASM release builds, and the browser ammunition suite. Browser checks include actual MG barrel dents and explosive ignition, slower cutting, shared six-case controls and exact saved replay.

**Previous: initial ammunition implementation (balance superseded above)**

Implemented the requested four ammunition types; the accepted wall, anchored rock and ordinary-barrel profiles remain unchanged. Public `Ammunition` and `Shot.ammunition` identify every shot; `Shot::new(ammunition, target)` supplies defaults. Existing explicit shot recipes/tests now name HE. HE keeps 6 kg / 220 m/s / 300 kJ blast / 1.7 m radius / 0.45 s reload. AP defaults to 8 kg / 350 m/s (490 kJ kinetic), with a 40 kJ / 0.65 m blast. MG uses 8 g / 600 m/s (1.44 kJ), no blast, a 4 mm sweep and 12 rounds/s. Physical shots retain bounded kinetic work and equal/opposite launch recoil; all are consumed at impact. AP does not yet continue through material.

Laser uses a first-surface ray, 40 m reach and 30 pulses/s. One pulse either subdivides the struck triangle into four connected children or removes one leaf of area at most **0.0125 m²**. It works on brittle and deformable material, reveals the local cutting grid, preserves surviving rigid velocity, and rebuilds colliders/connectivity. No recoil, impact loads, crack work or explosive ignition is generated. Supports/boundaries block the beam. The existing eight-generation / 8192-record limits apply; oversized cells are never erased to evade the cap. Ground anchors inherit only into the containing child. Cut counters/area are separate from delayed cleanup. Refinement transfers existing debris lifetimes to children and clears the retired parent marker; laser deletion also clears its cleanup marker, preventing duplicate removed-area accounting or renewed lifetimes. A completed cut can sever frames and detach ground-supported material.

Sandbox: **5/6/7/8** or HUD slots choose HE/AP/MG/laser, **B** cycles, hold click/F for MG/laser. Automatic fire is requested at fixed ticks, independent of rendered frames. Weapon switches retain the shared cooldown; paused input queues only one shot. The new read-only diagnostics include ammunition, cut count and cut area. `tools/ammunition-smoke.mjs` exercises these controls and comparisons.

Experiments add optional `impact.ammunition` (old JSON defaults to HE). HE/AP fire once; MG/laser hold for the test duration. Mass/speed validation and sweeps follow ammunition ranges; laser fields are hidden. The shared six-view bar has an AMMUNITION cycle control, and focused IMPACT has **AMMO … >** plus the existing copy-to-all action. Checkpoints record an optional beam and removed-area metric; exact saved replay is tested. Engine revision is **`experiment-ammunition-11`**; all five portable barrel studies are regenerated with their accepted historical settings/favorites.

`cargo run --release -p sim --example ammunition` measures a two-second trial: 24 MG rounds damage a 2 cm panel (23 cracks / 5 chunks), with no cracks in the accepted wall/rock and no barrel deformation. Laser produces zero kinetic energy in these unloaded trials. HE still gives the accepted wall 14 chunks and rock two chunks. See [AMMUNITION.md](AMMUNITION.md) for controls, profiles, persistence and limits.

Verification: **85 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native/WASM release builds. New ammunition browser checks pass, including shared six-view ammo changes without reload and exact saved laser replay. Existing gameplay and barrel-shot browser suites also pass. Inspected the ammo HUD, laser cut, focused comparison and six-view controls. Screenshots are `target/screenshots/ammunition-*.png`.

**Previous: corrected wall and anchored rock both accepted**

The user confirmed **“Yes, wall is good now”** after the correction below. Keep this corrected wall profile as the accepted default, alongside the previously accepted rock and barrel. Further tuning should start from these values; the earlier tougher wall was rejected.

The user reported that the wall had become more resistant instead of less, while confirming **“The rock is good now!”** The previous wall implementation had mistakenly increased seam strengths 16× and added a second support. Removed that increase, restored the earlier **0.7 × 0.6 m** brick layout and **single edge-2 support**, and retained cohesion **4** (earlier original wall was 18). Current wall strengths are **88 / 400 / 64 kPa**, density 2400, depth 0.55, friction 1.3, Gc 55, shell/contact resistance 1/1.5. The accepted rock, barrel and common physics are unchanged.

Measured centered standard input at one second: **14 chunks / 34 cracks / 40% largest area** for one 6 kg, 220 m/s shell; **4 chunks / 15 cracks / 47.5% largest area** for one 4500 kg, 4.6 m/s tank approach. Neither leaves anchored wall material. Three centered shells yield 20 chunks. Merely removing the strength increase left the two-support wall intact under tank contact, so the earlier support and brick layout were restored as well. The generic editor still supports multiple pins and ground attachments. Regression checks now require first-shot splitting and coarser first-ram rubble; the old test requiring the first hit to leave the wall standing was the wrong acceptance criterion.

Engine revision remains **`experiment-ground-anchors-10`**: this is a starter-recipe correction, not a solver change. Reload the application and select **O → L → Wall** (or rebuild the object yard) for the new profile. Existing saved drafts, custom copies and experiment recipes keep their authored values.

Verification: **76 sim + 2 camera tests passed**, formatting and clippy with warnings denied, native/WASM release builds, and browser library checks including actual first-shot wall breakup. Multi-support tests now author the second pin explicitly instead of relying on the starter default. Browser capture: `target/screenshots/library-wall-shot.png`.

**Previous: rejected tougher wall; accepted ground-anchored rock**

The user requested wall resistance similar to the original first-example barrier, and rock projectile response similar to the former wall, with anchoring in the middle so the tank cannot push it. The implementation assumed `Scene::Masonry` was the resistance reference; the user subsequently rejected the resulting tougher wall. Do not restore that wall tuning. `examples/obstacles.rs` preserves the former wall recipe and compares both references with the new starters; `--oblique` alternates shell targets.

Wall now has **two short-end supports**, **0.45 × 0.45 m bricks**, **cohesion 4** and seam strengths **1.408 MPa tension / 6.4 MPa compression / 1.024 MPa shear** (16× earlier values). Density 2400, depth 0.55, friction 1.3, Gc 55 and shell/contact resistance 1/1.5 remain. The first centered standard shot leaves it attached with eight cracked seams, comparable to the original barrier’s 98% anchored area; three shots yield 22 chunks. Three standard tank rams produce coarser rubble. Simply increasing the strength/work resistance multiplier was insufficient; support layout and brick size matter.

Rock uses the former wall’s **88/400/64 kPa seams, E 15 GPa, Gc 55, cohesion 18**, with **0.9 × 0.75 m stones**, contact resistance **1000**, and a **central ground attachment**. Its 2900 density / 0.65 depth / 1.1 friction remain. A centered standard shot chips ~0.53 m², leaving an anchored 89% core; head-on and ±0.6 rad five-second tank trials keep all material fixed. This is a material-response match, not identical trajectories or fragment counts across different shapes/supports. The accepted barrel settings and original example scenes are unchanged.

New optional asset fields: `ground_anchor: bool` (old files default false), `additional_pinned_edges: Vec<usize>` (old files default empty); existing `pinned_edge` remains. `ObjectMesh` computes an interior center and central grain set: grain centroids within one quarter of the smaller bounding dimension, plus the center-containing grain. The central stones provide a finite base rather than a single highly stressed node. Ground points are stored in piece-local coordinates. Rigid chunk assembly fixes connected material, stress nodes use the same constraints, and plastic mesh projection pins attached vertices. Adaptive refinement transfers each attachment only to its containing child, preserving world position. Detached chips become dynamic; isolated sub-threshold anchored crumbs can fade instead of becoming immortal obstacles. `BodyView.fixed` supports measurement.

SHAPE now shows **GROUND ANCHOR** below the edge-pin control, plus a center cross/circle guide. The existing pin button toggles each edge independently. Insertion removes the edited support and remaps remaining indices; deletion clears edge pins. Duplicate/out-of-range supports are rejected. Browser library checks cover both wall pins, undo, rock anchor toggle and reload. Native coverage adds anchor transfer/mass/cleanup, oblique tank resistance, the original-barrier comparison, multi-pin/legacy validation and exact saved experiment replay.

The oblique three-shot check (center, +0.65 m, −0.65 m) leaves five rock chunks and a **41.4% anchored core**, confirming progressive perimeter chipping as the aim changes.

Verification: **76 sim + 2 camera tests passed**, plus the strengthened anchored-core check, formatting, clippy with warnings denied, native/WASM release builds, browser library and editor suites. Inspected both new editor captures for readable controls and attachment markers. All five portable barrel recipes were regenerated.

Current engine revision: **`experiment-ground-anchors-10`**. New starters are used in the library and object yard; saved custom objects retain their own authored parameters. Stored experiment checkpoints retain their appearance until RUN. All five portable barrel studies are regenerated for the revision.

**Previous: stronger rotational friction and selected wall-release case 2**

The user has confirmed the combined result and explicitly requested it as the default barrel. Treat **wall-release case 2 + fourfold ground rotational friction + deformation limit 3** as the accepted ordinary-barrel baseline. These settings already apply to the library starter and object yard.

The user chose **number 2** from the six-case wall-release study. The ordinary barrel starter now uses **soft restitution 0.62**, **soft speed 16 m/s**, **base restitution 0.04** and **friction 0.09**. Its accepted deformation profile remains yield 500 Pa, flow 2/s, hardening 0, strain limit 3. This is an ordinary-barrel preset change; the generic soft-bounce default and other starters are unchanged.

`barrel_release_comparison()` marks index 1 as favorite, and the direct comparison opens that case selected. The six material values are unchanged. The portable `web/experiments/barrel-release.json` recipe is regenerated with the favorite. Existing saved user objects retain their authored values. The user then requested more rotational friction for all objects. `World::ground_resistance` now uses **10 rad/s² at friction 0.32**, four times the former 2.5. This applies once per dynamic non-tank chunk, including authored objects and fragments, and scales with its material friction. It clamps at zero and does not reverse rotation or add energy. Linear drag retains its existing coefficient; tank track traction remains separate. A friction-0.09 barrel spinning at 4 rad/s settles in ~1.42 s without external impulses. Engine revision is now **`experiment-angular-friction-9`**. All five portable studies are regenerated; saved user checkpoints must be rerun to show the new physics.

The existing floor-resistance regression now checks both spin directions, zero friction, low-friction stopping and kinetic-energy bounds. The broader replay test exposed a one-ULP JSON parsing error in an f64 fracture-work metric (`390.10337233543396` became `390.1033723354339`). Enabled serde_json `float_roundtrip` so saved checkpoints preserve exact metrics, retaining the strict replay assertions.

Verification: **71 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native and WASM release builds. Browser shot checks passed, including selected/favorite case 2, wall separation, free movement, exact replay and material editing. The selected barrel now ends about **0.620 m** clear of the backstop in the default shot; its free-shot motion matches the constant-bounce reference within the existing 1 mm / 0.001 rad bounds on this build.

**Previous: barrel separates from the wall while retaining the free-shot response**

The user clarified why backstop case 6 was preferred: **“It doesn't stick to the wall.”** The goal is wall separation while keeping the no-backstop behavior of case 2. Constant restitution changes could not achieve both in the measured recipe. Added a fixture-independent, incoming-speed-dependent material response (`ObjectAsset.soft_bounce: Option<SoftBounce>`). The ordinary barrel now uses **friction 0.09**, **base restitution 0.04**, **soft restitution 0.64**, **soft speed 16 m/s**. Below 16 m/s use the soft coefficient; from 16 to 32 interpolate with cubic smoothstep; at/above 32 return the base coefficient exactly. This is an effective game material curve, not calibrated metal plasticity or a full elastic recovery model.

`world/contacts.rs` supplies Rapier `MODIFY_SOLVER_CONTACTS` hooks only for opted-in colliders. It measures relative normal incoming speed at world contact anchors (including rotational velocity), evaluates each contacting material, and uses the existing average combine rule. No outward kick, adhesion workaround, fixture detection or floor-friction change. The property follows pieces/colliders through chunk rebuilds and adaptive refinement. Missing `soft_bounce` preserves constant restitution for old assets; the explosive barrel, fence, walls and rocks keep their earlier behavior. The accepted permanent-deformation solver and its yield/flow/hardening/limit-3 profile are unchanged.

Measured default shot: the tuned barrel ends ~**0.628 m** clear of the close backstop instead of ~0.003 m for old case 2. Across the default three-second no-backstop recording, position/angle differences versus old case 2 are constrained below **0.001 m / 0.001 rad**, including the hard arena-wall collision. This is a checked recipe, not a guarantee that all speeds, angles or object shapes match old behavior. Each recipe still replays exactly on the same build.

**`?experiment=barrel-release`** / native **`--barrel-release`** opens six soft coefficients **0.60 / 0.62 / 0.64 / 0.66 / 0.68 / 0.70**, with shared impact controls. **Case 3** is the new barrel default and starts selected; no favorite is attributed to the user. Earlier reference links (`barrel-shot`, `barrel-shot-free`, `crush`, `barrels`) explicitly retain their original constant-bounce materials and favorites. The new study is available by direct link; the editor picker keeps its existing buttons.

Editor **MATERIAL → BOUNCE >** opens a dedicated contact page with friction, base bounce, soft-impact toggle, soft bounce and soft-speed controls. Comparison **FOCUS CASE → MATERIAL → BOUNCE RESPONSE >** edits the same fields without rebuilding. Editor object-behavior button is narrowed to make room for the bounce-page shortcut. Old core field positions are retained; comparison help is moved down below the contact-page button. JSON validation enforces coefficients 0–1 and soft speed 0.1–100 m/s.

Current engine revision: **`experiment-soft-bounce-8`**. Export new study with `cargo run --release -p sim --example barrel_comparison -- --release`. `barrel_contact` measures constant restitution; `--soft` varies transition speed; `--gentle` measures the six new coefficients with/without the backstop. Native tests cover wall release versus the reference, near-identical free-shot recordings, exact serialized replay, contact-speed response in both directions, collision energy bounds and refinement inheritance. The extended browser shot suite checks the new study, separate material editing, validation and editor undo/persistence.

Verification: **71 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native/WASM release builds. Browser shot, library and editor suites passed. All five portable recipes were regenerated for this revision. Inspected the six-case overview and both bounce-control pages; corrected truncated editor help text.

**Previous: two preferred shot references and visible arena walls**

The user prefers **case 6 with the backstop** (friction 0.40, restitution 0.70) and **case 2 without it** (0.09, 0.04). Preserve both as tuning targets; this is not approval for context-dependent material switching. `barrel_shot_comparison()` now marks case 6; new `barrel_free_shot_comparison()` keeps the exact same six materials, disables the close fixture and marks case 2, under a distinct experiment name. Ordinary barrel defaults remain friction 0.09, restitution 0.04, accepted deformation limit 3. No physics solver change.

Investigation found that the no-backstop shot reaches the outer arena wall around 0.3 s (mean cell X ~9.57 m at 0.25 s, then reverses after the right wall). The comparison recorder skipped `Material::Boundary`, making real collisions appear to happen in empty space. Record the four actual boundary bodies with material ID 5, render in the muted fixture color and exclude them from `largest_area`. This makes their geometry/poses part of saved checkpoints. Existing pre-fix checkpoints still load as recorded; RUN adds visible walls. Engine revision **`experiment-boundary-7`** identifies the recording change. The shared shell toolbar says **OUTER WALLS ACTIVE** with the backstop off. It does not remove or alter arena walls.

Open **`?experiment=barrel-shot`** for preferred case 6, **`?experiment=barrel-shot-free`** for preferred case 2; native flags are **`--barrel-shot` / `--barrel-shot-free`**. No extra editor menu button. The existing backstop toggle remains available; parameter edits invalidate the old favorite as usual. `barrel_comparison --shot` exports `web/experiments/barrel-shot.json`; add `--free` for `barrel-shot-free.json`, both with one-second checkpoints and the appropriate favorite. Native coverage checks recorded walls against actual collider-body positions, intact barrel area, and exact serialized free-shot replay. Browser shot coverage verifies both direct routes, favorites and boundary records.

An optional clarification was asked about what the user liked in backstop case 6 (close settling, initial rebound, or shape/rotation); no response was available during this increment. Do not assume that simply splitting floor and collision friction can reproduce both results: the two recipes have different contact timing and collision histories. Current friction still controls both floor drag and surface contact; restitution does not affect free flight before a collision.

Verification: **69 sim + 2 camera tests passed**, formatting, clippy with warnings denied, native/WASM release builds. `tools/barrel-shot-smoke.mjs` passed including both favorite routes, saved wall geometry and exact replay. Four portable recipes (shot, free shot, crush, ten barrels) were regenerated. Inspected `target/screenshots/barrel-shot-free.png` for visible arena edges and the selected case.

**Previous: selected barrel profile + shell friction/bounce comparison (2026-09-13)**

The user chose **case 6, strain limit 3** from the accepted rim-only deformation study. `LibraryPreset::Barrel` now has yield **500 Pa**, flow **2/s**, hardening **0**, limit **3**. `crush_comparison()` uses this profile and marks case 6 as favorite. Generic `Plasticity::default()`, the explosive barrel, saved user assets and the deformation solver were not changed. In particular, do not restore the rejected contact-corner/convexity guard.

The next request was to compare barrel **bounciness and friction during a shot**. Added `Experiment::barrel_shot_comparison()`: six cases, columns friction **0.02 / 0.09 / 0.4**, rows restitution **0.04 / 0.7**, accepted deformation profile everywhere, identical centered **6 kg / 220 m/s** shell, backstop, **360 ticks / 3 s**. Starts looping slowly. Direct browser **`?experiment=barrel-shot`**, desktop **`--barrel-shot`**, editor **O → L → SHOT RESPONSE / 6**. This occupies the former ten-barrel shortcut; old `?experiment=barrels` / `--barrels` remain. No friction/bounce winner has been selected; ordinary barrel still uses **0.09 / 0.04** (case 2).

Added authored `ObjectAsset.restitution` (0–1; legacy missing field defaults 0.04), persisted into `Piece` and Rapier colliders, including chunk rebuild and adaptive refinement. Editor MATERIAL has friction/bounciness; comparison MATERIAL appends bounciness on row five, preserving earlier material field positions. Main editor basic material rows now have 36 px spacing. Compact cards display both coefficients. Restitution uses Rapier's existing average combination rule (backstop 0.08); shells are still consumed/explode on hit. Friction remains both collider friction and horizontal floor resistance.

All shared test controls work with the new matrix; editing one material leaves the other five untouched. Engine revision **`experiment-contact-6`** records the new property. Native tests cover legacy object defaults/range validation, inheritance through actual shot-driven refinement/fracture, differing rebound travel, and exact six-case serialized replay. Browser coverage is `tools/barrel-shot-smoke.mjs` (shared shot edits, one-case material edits, invalid value repair, editor draft persistence, library route, high DPI). Portable recipe command: `cargo run --release -p sim --example barrel_comparison -- --shot`; output `web/experiments/barrel-shot.json`, checkpoint tick 120. Existing `--crush` and no-flag recipes remain.

Verification for this increment: **68 sim + 2 camera tests passed**, clippy with warnings denied, formatting, native release and WASM release builds. Browser suites **barrel-shot, crush, library, editor and barrel** passed. Inspected overview, focused material controls, editor and high-DPI captures. All three portable barrel recipes were regenerated for `experiment-contact-6`. The older barrel smoke test now restores its ten-case recipe through the experiment library because the starter shortcut opens the new shot comparison.

The project is a working Rust destruction sandbox with native and browser builds, a tracked tank, and a destructible-object editor. The accepted physics rewrite replaced soft joints between material cells with rigid chunks and a separate elastic stress solver, and added adaptive subdivision on impact. The user tested this version and said: **“yes, now it feels better.”** Treat this as the accepted baseline.

The next requested increment is now implemented: editable grain layouts and a comparison range for reproducible experiments. The user explicitly chose **“Save reproducible experiments and replay them for comparison.”** The user subsequently requested a less crowded editor menu; that UI follow-up is described below. The accepted physics rewrite above remains the behavioral reference.

**Previous increment: six material variants with shared test controls**

The user asked which parameter controls the amount of deformation, wanted variants of that parameter, and asked to change speed/angle in the running simulation. `Experiment::crush_comparison()` now compares **strain limits 0.4 / 0.8 / 1.2 / 1.6 / 2.2 / 3.0** under identical impacts. All other object properties match (yield 500 Pa, flow 2/s, hardening 0, ~72 kg). Default test: 4500 kg tank at 0.8 m/s, centered, angle 0°, right-side backstop, eight seconds. This replaces the built-in speed/overlap matrix; previously saved experiments remain available. Open **`?experiment=crush`**, desktop **`--crush`**, or **O → L → DEFORMATION / 6**. Case 3 retains the original soft profile's 1.2 cap. The deformation solver stays exactly as restored after the previous negative feedback; no contact-corner constraint was reintroduced.

The **TEST ALL CASES** toolbar is visible above compact comparisons (more than four cases). It edits speed, angle, offset, mass and duration, cycles center/upper-half/lower-half track overlap, toggles the backstop and provides **RUN ALL**. Changes happen in the loaded app; no compilation/reload is needed for parameters. Material edits in FOCUS CASE remain independent, and MAKE 6 VARIANTS preserves six cases with factors 1/3, 2/3, 1, 4/3, 11/6 and 2.5 (bounded). Shared studies offer object-property sweeps; other comparisons retain independent impact sweeps. Generic comparisons still start with four cases. Strain limit is the maximum accumulated equivalent strain per cell, not a fraction of visible width; yield, hardening, flow and actual force/contact/time also matter. Native default final widths: **1.020 / 0.910 / 0.830 / 0.761 / 0.698 / 0.636 m**, from 1.30 m.

Recipes add optional `shared_impact` (default false for old files) and `impact.angle_degrees` (default 0). Shared recipes validate identical impacts and keep the flag on save/load. Test edits propagate across cases, including through the focused impact inspector, while object edits stay local. RUN ALL links an older independent experiment to the selected impact; ordinary RUN preserves independent impacts. Replaying unchanged shared settings retains its favorite. Tank angles −180°..180° rotate the actual initial pose and velocity; positive angles turn down on screen. At nonzero angles, the start distance comes from the object's projected outline, and offset is perpendicular to travel. The backstop stays on the right; initial overlaps at angled starts reject the recipe with a repairable message. Shell mode shows target X/Y instead of tank angle. Zero-angle construction preserves the previous initial conditions and deformation behavior.

At that stage, engine revision **`experiment-angle-5`** recorded the new test setup; old checkpoints remain readable. Portable recipes `web/experiments/{crush,barrels}.json` are regenerated by `barrel_comparison` (add `--crush` for six limits). Native tests cover identical loading with separate limits, real rotated pose/velocity, offsets, shared validation, legacy JSON defaults and exact angled replay. The new browser suite checks shared runtime edits without reloading WASM, limits, separate material edits, six-case sweeps, invalid-angle repair, saved favorites/checkpoints and exact replay. See [EXPERIMENTS.md](EXPERIMENTS.md) and [PLASTICITY.md](PLASTICITY.md).

**Previous feedback: restore the previous deformation, keep the new cases**

After testing the speed/partial-impact update, the user said **“no, the deformation was better before.”** The contact-corner constraint introduced in `plastic-frame-contact-3` has been removed completely. `Mesh` again projects only the authored rim edge lengths; there is no contact-side convexity/flatness guard. This restores the deformation implementation from `plastic-frame-rim-2`, including inward folds. The six-case barrel's strain limit is also restored from 3 to **1.2**; profile name is now `Barrel / soft frame`, yield 500 Pa, flow 2/s, hardening 0. Do not treat the rejected flat-wall/deep-crush response as the accepted baseline. Existing mass, momentum, work, perimeter and geometry constraints remain in place. User-authored drafts/materials and saved checkpoints were not rewritten.

At the rollback stage, the testing additions were: **`http://127.0.0.1:8000/sandbox.html?experiment=crush`**, native **`--crush`**, or **O → L → SPEED / HALF HITS**. `Experiment::crush_comparison()` gives **six cases: rows 0.8 / 3 m/s, columns full / upper half / lower half**, eight seconds each, with the same 72 kg soft barrel, 4500 kg tank and backstop. The tank lane moves by −0.82 / +0.82 m (screen up/down), putting a track edge on the barrel's initial midline. Actual contact drives folding, rotation and slip; overlap can change during a crush. Focus → IMPACT retains numeric speed/offset and FULL / UPPER 1/2 / LOWER 1/2 presets; compact cards show actual speed/offset, and camera fit accommodates the offset tanks. All cases remain editable, saveable and replayable, with favorite selection and JSON v1. The original ten-case yield/hardening study remains available and its profiles are unchanged.

Rollback engine revision **`plastic-frame-rim-4`** identifies that rollback, so saved contact-3 checkpoints trigger the version notice instead of silently being treated as the same physics. New RUN uses current physics with the saved recipe's material values; opening the prepared study uses the restored 1.2 cap. Both portable recipes are regenerated. `cargo run --release -p sim --example barrel_comparison -- --crush` exports `web/experiments/crush.json` with a six-second checkpoint; without the flag exports the ten-case recipe. Removed the test demanding a flat wall-facing rim/deeper flattening; retained physical invariants and six-case exact replay checks. Browser tests now expect the original profile and deformation with retained perimeter. See [EXPERIMENTS.md](EXPERIMENTS.md) and [PLASTICITY.md](PLASTICITY.md).

**Previous correction: transverse expansion under crushing**

The user observed barrels shrinking across the applied force. The prior rest-shape model preserved mass but allowed the frame edges to contract. `Piece.reference_edges` now retains authored triangle edge lengths. `Mesh` identifies exterior edges independently of grain-rendering masks and projects every candidate onto those edge-length constraints using welded vertex DOFs, support pins and hanging-vertex stencils. It performs up to 96 nonlinear sweeps, rejects residual rim errors above 0.002%, and backtracks when necessary. Length targets always refer to the authored geometry, preventing accumulated shrinkage. Free candidates are aligned to the old mass center/orientation after projection; work, strain caps, 15 mm motion caps, collision checks and inertia/momentum updates include the complete corrected candidate.

This is a non-stretching frame approximation, not an incompressible filled polygon or full shell model. A squeezed barrel widens across the force, while its hollow interior may lose area. Fence and arbitrary deformable exterior frames use the same constraints. No new UI controls or JSON fields were added; the engine revision is now `plastic-frame-rim-4`. Existing visual checkpoints still load as saved; rerunning uses corrected physics. The ten-case portable recipe is regenerated for this revision. Native tests cover three force directions × two mesh resolutions, retained rim lengths and actual tank/backstop widening, plus existing conservation/unloading checks. The browser barrel suite checks width, height and perimeter in every case. See [PLASTICITY.md](PLASTICITY.md).

**Previous follow-up: permanent deformation, separate from fracture**

The user asked for permanent non-elastic deformation, then explicitly allowed two object classes: deformable frames and destructible objects. Implemented `Components.plasticity: Option<Plasticity>`: `Some` keeps the authored frame connected and deforms it permanently; `None` retains the existing rigid-chunk fracture/refinement behavior. No combined tearing mode. Barrels (including explosive) now deform; walls/rocks remain destructible. Added blue `LibraryPreset::MetalFence` on the library's second starter page and in the object yard.

`world/plasticity.rs` projects yielded interface rest offsets through the cached structural system, welds vertices across intact bonds, pins supported vertices and resolves hanging-vertex constraints. Shape steps are rate/work bounded, preserve reference mass and free-body momentum, and backtrack on inversion, overlap or increased external penetration. Both triangles and colliders change. There is no elastic recovery; unloaded shapes are unchanged. Hardening raises yield; max strain stops further change without tearing. Deformable objects cannot fracture or adaptively refine. An explosive can ignite on accepted deformation; no fake hit-point damage is used. See [PLASTICITY.md](PLASTICITY.md) for equations, scope and controls.

Material → YIELD & HARDENING is a direct shortcut for deformable assets. Any asset can switch mode through OBJECT BEHAVIOR → PERMANENT DEFORMATION. Relevant fields are yield kPa, flow /s, hardening and strain limit; fracture-only controls are hidden while their recipe values are retained. Components survive undo/draft/library/export. The frame is the authored triangular mesh, not a separately authored shell/beam skeleton. Defaults are effective game values; barrel area may compress while reference mass stays constant. Full shell FEM, metal incompressibility, elastic springback and tearing are not implemented.

Each piece has `reference_area`, `plastic_strain`, `shape_revision`; `BodyView` exposes the latter two. Every rest-shape revision gets immutable recording geometry, preventing new dents from rewriting earlier replay frames. Metrics add plastic strain/work with serde defaults. Old object JSON without plasticity stays destructible. Saved visual checkpoints retain deformed geometry; RUN reconstructs the recipe. Current engine revision is `experiment-angle-5`.

The ten-barrel study now sweeps yield **0.5/1/2/4/8 kPa** across columns and hardening **0/4** across rows. Same 72 kg body, 4500 kg tank at 0.8 m/s, 15 cm backstop gap, four seconds. Final peak strain spans about 15%–120%; all ten remain one chunk with zero cracks/failed solves. Generic deformable comparisons default to a yield sweep and expose yield/flow/hardening in the inspector/sweep menu. Let the user pick and tune the preferred response.

Open `http://127.0.0.1:8000/sandbox.html?experiment=barrels` or **O → L → COMPARE 10 BARRELS**; desktop accepts `--barrels`. Direct launch starts looping playback without changing the editor draft or saved experiments. `web/index.html` recognizes comparison startup when dismissing the loading panel. The range/schema accept 1–10 cases; five-column/two-row overview hides the inspector until FOCUS CASE. Click a case, MARK SELECTED FAVORITE, SAVE; in focus mode USE CASE IN OBJECT EDITOR copies the exact profile. LOOP is a playback control. Existing four-case sweeps remain available.

Optional `Impact.backstop` defaults to false for old JSON; the runner adds a rigid unbreakable fixture through `World::add_test_backstop`. Impact inspector can toggle/copy it. Current engine revision is **`experiment-angle-5`**. `web/experiments/barrels.json` is a portable recipe with a 1.5-second review checkpoint, regenerated by `cargo run -p sim --example barrel_comparison`. Browser suite `tools/barrel-smoke.mjs` checks ten cards, direct and library launch, focus, exact replay, saved favorite and high-DPI. Screenshots: `target/screenshots/barrels-ten.png`, `barrels-start.png`, `barrels-focus.png`.

**Previous increment: object library**

The user requested basic editable barrel, explosive barrel, wall and rock components, with testing/calibration as the next iteration. Initially implemented four colored `LibraryPreset` recipes separately from legacy `ObjectPreset` fixtures; optional `ObjectAsset.color` and `components` retain JSON v1 compatibility. `object/library.rs` owns definitions, validation and saved-copy schema. `world/components.rs` handles object instance identity, opposed-contact crush resistance, one-shot ignition on actual internal seam failure or accepted plastic deformation, and a shared object yard. Refined children retain color/identity/components. Exploding vessels are consumed before the existing occluded blast rays are cast. The initial library used brittle fracture; barrel behavior is superseded by the permanent-deformation follow-up above.

Editor **L / FILE → OBJECT LIBRARY** opens starters and saved copies, **SAVE CURRENT** upserts by name, **SHAPE → COLOR** takes hex/AUTO, and **MATERIAL → OBJECT BEHAVIOR** exposes shell/contact resistance, crush force/scale, explosive energy/radius and contact refinement. The sidebar still has three task tabs. **4 / 04 OBJECTS** or **TEST OBJECT YARD** opens a scene with both barrels, masonry, rock and a backstop. **TEST** uses the edited draft; the yard uses built-ins. Native `World::from_objects` supports validated multi-object fixtures (32 objects, 8192 initial piece records). The old three `Scene::ALL` regression fixtures are unchanged; ObjectYard is appended in UI navigation.

Custom library: `objects/library.json` or browser key `fracture.objects.v1`, 24 copies / 1 MiB. Old unreadable saves and failed writes are preserved. Individual files use existing object import/export. Full object definitions and colors survive experiment recipes and recordings. Engine revision bumped to **`object-library-1`**. New browser suite `tools/library-smoke.mjs` restores both original object stores. Native example `cargo run --release -p sim --example library` reports basic shell/tank trials. See [OBJECT_LIBRARY.md](OBJECT_LIBRARY.md) for exact initial profiles, controls and known limits.

The rock now chips under a default shell, keeps more than 60% of its area in one core, and stays intact in the basic tank trial. Wall trials produce larger chunks under tank contact than shell impact. A free barrel can be pushed at low speed without cracking, while opposed backstop contacts permit permanent deformation in the current barrel profile. Explosives ignite once, consume their geometry and can fracture a nearby weak panel. These are narrow regression cases, not a finished calibration. Next: use reproducible comparisons to tune shape, grains, repeated shots, impact resistance, crush thresholds and useful blast damage distances with the user.

**User's intended game and preferences**

- A top-down 2D, potentially isometric-looking tank/machine battle game inspired by older tank games. The user is a software engineer who mainly works in Rust, rather than a game developer.
- Physics and satisfying material destruction are the priority. Movement must be continuous, without a placement/movement grid. Objects have physical geometry and material properties; there are no abstract health points.
- Minimalist lines and shapes on a dark background, with bright edges and readable, contrasting text. The user rejected the early beige, thin-font, dashboard-like interface. No environment textures or sprites are needed.
- The tank should feel heavy, with useful traction and braking. Separate left/right track control should remain convenient. Shells should feel like ammunition, explode on impact, and disappear rather than bounce around as balls.
- Rocks should leave substantial chunks. One impact should not disintegrate the whole field. Large leftover triangles must be breakable further, instead of becoming permanent uncleanable debris.
- Browser support matters and is already implemented. Multiplayer is a later stage. The editor explicitly focuses on destructible objects first; machine assembly was deferred.
- The user authorized the substantial physics rewrite (“the hard but proper way”) and has now accepted its improved feel. Full FEM was an early possibility to investigate, not a requirement to claim as implemented.

**Development so far**

1. Chose Rust, Rapier 2D for collision/dynamics, Macroquad for native/browser presentation, and a custom destruction layer. Kept the simulation independent of rendering.
2. Built three repeatable scenes: an anchored masonry barrier, a panel with a narrow cantilever neck, and a field of breakable rocks. Added fixed stepping, interpolation, pause, single-step, slow motion, stress display, and a launcher.
3. Reworked presentation into a dark wireframe battlefield with bold text and a field manual. Added a visible tracked tank, mouse-aimed turret, independent tracks, floor resistance, recoil braking, swept shells, impact explosions, and shell consumption. Smoothed zoom with wall-time easing and a stable cursor anchor.
4. Improved rock behavior with denser material, stronger grain interiors, weaker inter-grain seams, distance/occlusion-aware blast delivery, and cleanup based on connected fragment area.
5. Built the polygon object editor with material properties, mesh preview, supports, undo/redo, test/reset/return, autosave, and portable JSON import/export.
6. Replaced the per-cell soft-joint model because increasing detail made objects behave like rubber. Added rigid connected chunks, an elastic interface graph, bounded crack work, conservative splitting, and local adaptive refinement. Re-equilibrating after each crack removed an additional cascade cause. Cached sparse factorization reduced the cost of detailed stress solves. Source-history guards and mesh-independent blast resultants fixed further refinement/loading inconsistencies.

7. Added authored stone/Voronoi sites and staggered brick layouts, independent of solver detail, with dragging/add/delete/undo, deterministic regeneration and visible grain boundaries. E and Gc are editable. Short shared faces are collapsed globally before triangulation; concave ears are chosen for quality. Added 1–4 variant shell/tank experiments, nine parameter sweeps, synchronized recordings/timeline, favorite selection, editor round trips, local library and JSON import/export.

**Current technology and file map**

Rust edition 2024 workspace; Rapier 2D **0.35.3** with `enhanced-determinism`, Macroquad **0.4.16**, Serde/serde_json. Dependencies are pinned in `Cargo.lock`. The browser uses WASM/WebGL and bundled loader/font assets; no npm build or runtime CDN is needed.

| File / directory | Responsibility |
| --- | --- |
| [crates/sim/src/world.rs](../crates/sim/src/world.rs) | World, pieces/bonds, commands/events/views/stats, tick orchestration, cleanup |
| [world/chunks.rs](../crates/sim/src/world/chunks.rs) | Connected components, rigid-body rebuilding, mass/inertia and velocity inheritance |
| [structure.rs](../crates/sim/src/structure.rs), [structure/factor.rs](../crates/sim/src/structure/factor.rs) | Elastic interface system, sparse LDLᵀ factorization, residual checks and fallback |
| [world/loads.rs](../crates/sim/src/world/loads.rs) | Weapon/contact loads, inertial balance, stress failure, crack work and impulse work caps |
| [world/plasticity.rs](../crates/sim/src/world/plasticity.rs) | Yielded rest-shape projection, welded frame updates, collision guards, work and conservation |
| [world/refinement.rs](../crates/sim/src/world/refinement.rs) | Runtime subdivision, face clipping, support/crack preservation, source remapping and limits |
| [world/weapons.rs](../crates/sim/src/world/weapons.rs) | Swept shells, pressure rays/pulses, source budgets |
| [world/vehicle.rs](../crates/sim/src/world/vehicle.rs), [world/geometry.rs](../crates/sim/src/world/geometry.rs) | Tank/floor forces; scene and authored-object geometry |
| [object.rs](../crates/sim/src/object.rs), [material.rs](../crates/sim/src/material.rs) | Versioned assets, polygon validation/triangulation and material definitions |
| [world/tests.rs](../crates/sim/src/world/tests.rs), [world/rigidity_tests.rs](../crates/sim/src/world/rigidity_tests.rs) | World regression and conservation/refinement tests; other modules also have tests |
| [object/meshing.rs](../crates/sim/src/object/meshing.rs) | Grain partitioning, conforming seams, short-edge collapse and topology/area checks |
| [experiment.rs](../crates/sim/src/experiment.rs) | Versioned recipes, validation, synchronized runner, recorded geometry/poses and library schema |
| [crates/sandbox/src/comparison.rs](../crates/sandbox/src/comparison.rs) | Comparison UI, authoring, linked cameras, playback and library |
| [crates/sandbox/src](../crates/sandbox/src) | Input, camera, HUD, rendering, editor, persistence and read-only WASM diagnostics |
| [web/editor.js](../web/editor.js), [web/experiments.js](../web/experiments.js) | Separate object/experiment local storage and JSON file bridges |
| [tools](../tools) | WASM build, local server and browser/editor regression scripts |
| [crates/sim/examples](../crates/sim/examples) | `measure`, `impact`, and `resolution` scenarios |

**Physics model to preserve**

The plane is horizontal, with zero in-plane gravity. Units are meters, kilograms and seconds; effective depth converts polygon area into volume/mass. Simulation runs at **120 Hz with four microsteps**, each using one Rapier TGS step and 12 PGS sweeps.

A **cell** is internal material geometry with a collider and local pose. A **chunk** is a connected material component sharing one Rapier rigid body. An intact supported component is fixed; a detached component moves freely. Cells in the same body cannot collide with each other. Exact triangle tiling replaces the old seam clearance. There are no physical joints between cells.

The structural graph has three virtual degrees of freedom per cell: two translations and rotation. Shared faces use EA/L, GA/L and EI/L stiffnesses. Supports constrain displacement; free chunks subtract mass-weighted translational/angular acceleration before solving internal loads. Virtual elastic displacement never changes visible or collision geometry. The sparse system is cached in the chunk's local frame; topology changes invalidate it. A solve that fails its residual check cannot initiate cracks.

Failure requires a stress overload and available work for `Gc × face width × depth × cohesion`. The most overstressed affordable seam breaks, then the solver re-equilibrates. Each pass permits up to eight rounds. Choosing many cracks from one stale stress solution previously caused unnecessary disintegration. Splits preserve mass, linear/angular momentum and kinetic energy through inheritance of the parent's rigid velocity field; they receive no artificial outward kick.

Shell and blast packets carry bounded work budgets. Crack costs are deducted before positive kinetic work is applied, including rotational work. Rapier contact impulses have already acted and must never be applied again; their crack budgets are limited by estimated dissipation and actual total kinetic loss in the microstep. Blast ray vectors retain their net impulse and torque when grouped by receiving cell. Normalizing a grouped vector and replacing its magnitude with total scalar energy would make blast push depend on mesh detail again.

Default material modulus/fracture-surface work: rock 40 GPa / 60 J/m², panel 8 GPa / 120 J/m², masonry 15 GPa / 30 J/m²; Poisson ratio 0.2. E, Gc, density, thickness, friction, stress strengths and cohesion are editable. Optional `young` (Pa) and `toughness` (J/m²) fields preserve object JSON v1 compatibility. These are illustrative values. Cohesion scales internal-grain strength and fracture work.

**Refinement and disappearance rules**

Strong, energetic impacts can subdivide a triangle with area **at least 0.075 m²** into four bonded children, whether it is inside an intact structure, part of a detached chunk, or isolated. The eligibility heuristic compares impulse stress with half the cohesive tensile limit and requires enough work for a candidate face. Actual fracture still comes from the structural solve. Subdivision itself does not break material or consume crack work.

Refinement preserves area/mass, existing cracks, support attachments and total inherited face width. Pending loads and ongoing blast pulses follow their material point into a child. A lineage can refine only once per impact/pulse, but later impacts can refine it further. Children retain **all ancestral refinement source IDs**, not merely the most recent one: intervening contacts must not let an earlier still-active pulse recursively refine again.

Guards: at most eight cells refine per processing pass, eight runtime generations per lineage, and **8,192 retained piece/cell records per scene**, including retired parents and cleared debris. Reset reclaims the record budget. The field manual reports limits. These are safety guards, not fragment-capacity benchmarks.

After topology changes, an unanchored disconnected chunk whose **combined area is strictly below 0.075 m²** receives a 3.0–3.9 simulation-second lifetime, fading during its last second. Small cells within a larger component stay. Larger fragments stay until further damage makes smaller chunks. Cleanup deliberately removes mass and residual motion as a game rule. Stable cell IDs are not reused; descendants retain an original-cell ID for diagnostics.

`BodyView` still enumerates cells for rendering. `Stats.bodies` counts actual dynamic bodies, including the tank; `chunks` counts material components, including supported ones; `cells` counts active material cells. Do not restore old tests that assume one body per triangle. Compare debris **area**, rather than deleted-cell count, across adaptive meshes. Apply floor resistance once per chunk.

**Editor and controls**

Presets: rock, wall, plate, concave L-block. Edit outline vertices by dragging or coordinates; Shift-click inserts a vertex and Delete removes one. Edit density, depth, friction, tensile/compressive/shear strengths, initial detail 0–4, and cohesion. Pin the selected vertex's outgoing edge. Fresh workshop presets use stone/Voronoi or brick grains, detail 0 and cohesion 12. Existing assets without `layout` retain the legacy ear-clipped grains; `ObjectAsset::preset` deliberately remains the reference fixture and `workshop_preset` supplies new editor defaults. The optional layout stores mode, dimensions, seed and explicit stone sites. Grain interiors are triangulated separately, then detail subdivides fourfold. The SHAPE/FRACTURE tabs select outline or stone-point dragging, X/Y and add/delete targets. W/H or seed edits and REGENERATE replace manual sites.

Generated meshes dissolve construction diagonals, insert shared T-junctions and collapse faces shorter than 3 cm wherever the original boundary can be preserved. Convex grains fan from an interior point; concave grains use quality-ranked ear clipping. Validation rejects gaps, nonmanifold seams, inverted/degenerate cells and area changes. Very small clipped boundary triangles can still occur; the authored-layout minimum cell altitude is 0.5 mm versus 8 mm for legacy layouts. Arbitrary layouts are not quality-guaranteed; invalid edits remain repairable. Tests cover all presets × both modes × 16 seeds for coverage/joined edges, and the default generated presets for rest/impact stability.

Pieces carry optional grain identity and a visible-edge mask. Intact grain interiors are hidden by summing the covered width of active same-grain faces, including subdivided neighbors; broken faces become visible. Stress view shows solver triangles. This rendering mask does not change collision or failure mechanics.

Authoring limits: 32 outline vertices, 128 grains, 2,048 initial cells, 64 KiB JSON; no holes/disconnected outlines. JSON version **1 remains compatible**. Runtime fracture never changes the draft. Browser drafts use local storage key `fracture.object.v1`; native drafts use `objects/draft.json`. Native explicit import/export uses `objects/object.json`. Browser import/export uses a file picker/download. Preserve users' drafts during experiments; the editor smoke script restores the original local-storage draft.

W/S drive both tracks, A/D steer, Q/Z override the left track and E/C the right. Releasing controls brakes. Mouse aims, click/F fires. H opens the manual, O opens/returns to the editor, Enter tests an object, R rebuilds, Space pauses, N advances one tick, T slows time, Tab shows stress, 1/2/3/4 select scenes. Wheel zooms, right-drag pans, Home recenters.

Default shell: 6 kg at 220 m/s, 0.45 s reload, consumed at first impact. Blast: 192 occluded rays, 1.7 m default radius, 300 kJ default source budget and 35 ms pressure pulse. Tank: 4,500 kg, traction/braking through four ground contact patches. Tracks/turret are not yet detachable physical assemblies.

The latest UI feedback was that too many editor buttons were visible at once. The workshop now uses SHAPE / FRACTURE / MATERIAL sidebar tabs, FILE and VIEW dropdowns, and collapsed FAILURE & STIFFNESS / SOLVER DETAIL sections. Presets and import/export are in FILE; snapping, solver-mesh visibility and fit are in VIEW. TEST and COMPARE remain in the toolbar. The canvas is taller and defaults to showing grain seams without solver triangles. Point editing is restricted to Shape or stone Fracture; Material cannot delete geometry. Dropdowns consume clicks and Escape before they reach the canvas/editor navigation. Existing editor/comparison browser scripts use the new navigation and cover dropdown dismissal and contextual Delete.

**Comparison workflow and persistence**

Editor → COMPARE creates four strength variants. Select a case, edit MATERIAL/IMPACT/MESH, choose a sweep and MAKE 4 VARIANTS, then RUN. Shell and tank trials share 120 Hz time, with cached visual frames every four ticks. Space/play, rewind, scrub, N/frame, T/slow, linked wheel zoom/right-pan and focus mode are available. MARK FAVORITE is saved with the experiment; USE CASE IN OBJECT EDITOR copies its object. Editing a comparison case's mesh returns to the same experiment through COMPARE. Physical edits invalidate previous results.

SAVE keeps a named recipe and exact visible checkpoint. LIBRARY restores that frame immediately; RUN reconstructs the timeline. These are not engine-state snapshots. Experiment JSON v1 has `engine`, `variants`, duration/cursor ticks, favorite and review geometry/poses/metrics. Bump `ENGINE_REVISION` (`experiment-soft-bounce-8`) when changing physics or mesh behavior affecting replay. Same-build replay is tested natively and in WASM; do not promise identical results across code revisions/platforms.

Browser library: `fracture.experiments.v1`; native: `objects/experiments.json`. Native explicit file: `objects/experiment.json`. Up to 12 library entries and 8 MiB serialized bytes; browser quotas may be lower. Failures preserve previous saved content and report the error. The separate `web/experiments.js` bridge is required by the WASM build. The comparison smoke suite restores both original local stores.

The runner advances one case per call and only advances shared time after every case. The UI spends about 6 ms/frame calculating, but individual expensive steps can hitch. Immutable geometry revisions are shared across frames; roughly two million pose records (plus two pose-equivalents per new geometry revision) trigger a recording cap at a shared frame boundary. Experiments last 1–10 s and contain 1–10 cases. Peak elastic energy is diagnostic; uniformly scaling E can leave force-controlled fracture unchanged. Grain size, Gc, strength, mass and impact changes provide other comparison axes. See [EXPERIMENTS.md](EXPERIMENTS.md).

**Running and validation**

```sh
cargo run --release -p sandbox

# Browser build and server (install the Rust WASM target if needed):
bash tools/build-web.sh
python3 tools/serve.py
```

Open `http://127.0.0.1:8000`. Rebuild WASM and refresh after simulation changes. Native and WASM release builds were rebuilt at the end of the physics work. Processes and generated artifacts may not survive a new environment; do not assume the old server or Chrome is still running.

The shared-test/deformation-limit update passed **66 simulation tests + two camera tests**, Clippy with warnings denied, formatting, and native/WASM release builds. The crush, barrel and comparison browser suites passed, covering common runtime controls without a WASM/page reload, actual angled poses, saved shared checkpoints/favorites, exact replay, independent material edits and six-case sweeps. Plasticity browser tests last passed before the rollback; library passed during the preceding permanent-deformation increment, and generic browser/editor during the object-library increment. Screenshots `target/screenshots/crush-six.png`, `crush-shared-angle.png` and `crush-material.png` were inspected; the suite also captures start, library and high-DPI views. Browser checks restore existing local-storage values.

```sh
cargo test --workspace --locked                         # 66 sim + 2 camera tests passed
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build --release -p sandbox --locked
bash tools/build-web.sh
node tools/browser-smoke.mjs
node tools/editor-smoke.mjs
node tools/comparison-smoke.mjs
node tools/library-smoke.mjs
node tools/barrel-smoke.mjs
node tools/crush-smoke.mjs
node tools/plasticity-smoke.mjs
```

The original browser suites and the rerun comparison/library checks cover an isolated large triangle acquiring seams and breaking on impact, reset restoring its original single cell, draft preservation, editable grain points, brick dimensions, shell/tank experiments, exact recipe replay, saved checkpoints, library persistence, invalid imports and high-DPI rendering. Screenshots in `target/screenshots/` were inspected. Tests cover conservation, rigidity across detail 0–4, known stress reactions, source guards, supports/cracks, contact energy, blast resultants, rock retention and existing tank/editor behavior.

Browser scripts need Node 22+, the local server, and an isolated Chrome with remote debugging on port 9227. Launch instructions and environment overrides are in [SANDBOX.md](SANDBOX.md). Earlier checks used software rendering. New tool environments may require permission to access local server/CDP ports. No commit or PR was created during this development session; inspect version-control availability when resuming.

**Measured limits and remaining work**

Earlier measurements of the accepted baseline (legacy meshes), on Linux x86-64 / AMD Ryzen Threadripper PRO 7965WX: release `measure` (seven shots / 1,200 ticks) reported p95/maximum physics tick times: barrier **3.61/19.09 ms**, cantilever **1.19/6.51 ms**, rocks **0.14/0.66 ms**. These are individual native runs on a shared machine, not browser/mobile capacity claims.

For one breakable editor rock, 80 initial cells took **0.253/3.302 ms** p95/peak; 320 took **0.787/18.282 ms**; 1,280 took **4.283/180.318 ms**. All measured runs had zero failed stress solves. Extreme detail can still hitch badly during fracture because topology changes rebuild stress systems and collision geometry. Start with detail **0–2** and let impacts add local detail. Run `measure`, `impact`, and `resolution` examples as appropriate for future changes; full tables are in [SANDBOX.md](SANDBOX.md).

The current model combines separate quasistatic brittle fracture and effective rigid/plastic deformation, not full dynamic FEM or cohesive traction/separation. Crack work comes from source/dissipated-work budgets; virtual elastic energy is diagnostic and is not released into fragment motion. Cracks still follow existing or newly subdivided triangle edges. Failure patterns remain mesh/timestep dependent despite rigid-motion and conservation checks. There is no full continuum plasticity, tearing, fatigue, transient stress-wave propagation, sustained centrifugal stress, vertical falling/stacking, penetration, audio or multiplayer. Whole-chunk floor drag does not contribute an internal stress field. Same-build replay is tested; cross-platform bit-identical replay is not claimed.

Useful next directions, to select with the user: profile/reduce topology rebuild spikes on actual browser hardware; use the new experiments to calibrate material/mesh behavior; improve crack geometry; develop machine components and their functional failure; add sound/gameplay; eventually authoritative multiplayer. Preserve the accepted feel and physical invariants while extending the model. The old soft-joint approach and mass-dependent blast redistribution are superseded.

For further detail, read [DESTRUCTION.md](DESTRUCTION.md) for current physics, [EDITOR.md](EDITOR.md) for authoring/storage, [EXPERIMENTS.md](EXPERIMENTS.md) for comparisons, and [SANDBOX.md](SANDBOX.md) for validation/performance. [PLAN.md](PLAN.md) is the original design history, with proposals that are not all current implementation decisions.


## 2026-09-14 — Level editing, geometry joins and scale

- Shared terrain composition draws non-overlapping opaque fills and exposed edges only. Sand, water and bridge titles are hidden; A/B booth markers remain.
- Level editor supports Shift-click/box selection, group movement and centre coordinates, Ctrl A/C/V/X/D/J, undoable paste/cut/merge, and a session clipboard. Unique team markers can be moved by cut/paste but cannot be duplicated.
- Auto Join combines touching/overlapping matching rocks and walls into one polygon and one fracture mesh. Axis-aligned wall edges snap together; Alt suppresses snapping/joining. Unbreakable walls can also join. Hole-producing unions are explicitly rejected by the simple-polygon schema.
- Circular barrel constraints survive renaming, diameter edits and level scaling while live plastic deformation remains unchanged.
- Boundary and dimension drags clamp. Large objects automatically increase grain spacing, with 128 outline vertices available for merged shapes. Bridges have a dedicated conforming grid (up to 256 triangles, 16 of them in booths), with 3–190 m spans and 2–190 m lane widths, limited by map space. Unbreakable wall and block tools are explicit.
- Tank top speed increased from 4.6 to 6.5 m/s; road 9.425 m/s and sand 3.12 m/s. Existing traction/brake forces and saved experiment speeds remain.
- Added the pinned `i_overlay` crate for robust polygon booleans and a fallback for rare grain-boundary clipping failures. Existing successful mesh construction remains unchanged.
- Validation: 120 simulation and 4 sandbox tests; fmt, clippy, native/WASM releases; five relevant browser suites, including new interaction and geometry coverage.


## 2026-09-26 — Local game and audio/terrain checkpoint

Saved the local lobby/match flow and separate applications, plasma/cutting fixes,
WWII-style adaptive soundtrack, sound settings, native mixer repairs, quieter
material effects, small-object sight filtering, terrain movement sounds, CLIFF
object deflection and decorative destruction audio. See the
[checkpoint notes](checkpoints/2026-09-26.md) for the exact scope and validation.
Latest checks: 233 workspace tests passed (one existing ignored), formatting and
clippy with existing allowances, native/WASM builds, and the audio browser suite.
A source-and-assets archive is saved under `checkpoints/2026-09-26-game-progress/`;
Git was unavailable because this workspace has an empty, read-only `.git`.


## 2026-09-26 — Current workspace save

Saved a second source/assets checkpoint, preserving the earlier completed-game
archive. This snapshot includes the unfinished six-player simulation groundwork;
network transport and client/lobby integration remain outstanding. See
[current checkpoint notes](checkpoints/2026-09-26-current-progress.md).


## 2026-09-26 — LAN multiplayer headless acceptance phase

Added authoritative multi-tank simulation, desktop host/join and readiness flow,
UDP discovery, bounded binary/LZ4 TCP replication, persistent geometry and motion
deltas, per-player visibility and display-only interpolated client worlds.

At the user's request, desktop checks stopped in favor of headless validation.
`tools/check-lan-headless.sh` passed with 250 unit tests and four real-socket
scenarios, formatting and lint. Six-client River Valley movement/firing held
20 Hz snapshots and 120 Hz simulation without dropping time; see
[the report](LAN_HEADLESS_TESTS.md). Shared visibility preparation reduced snapshot
work from approximately 19 ms to 5 ms. Full multiplayer completion still requires
the remaining UI/build/browser acceptance checks.


## 2026-09-26 — LAN multiplayer acceptance complete

Following headless success, both native executables and both WASM executables built successfully. Existing game and audio browser regressions passed. Two native game processes verified direct/discovery joining, map/readiness updates, editor return, countdown, independent views, movement and shooting, non-pausing menus/settings, return to lobby and disconnect feedback. This caught a queued text-input leak: movement keys appeared in the name field after leaving a match. LAN entry and disconnect now clear that queue; rebuilt-client checks passed. See [the acceptance audit](LAN_ACCEPTANCE.md) for evidence and remaining measurement limits. No physical multi-computer LAN/Wi-Fi test was performed.


## 2026-09-26 — Apple Silicon build and text carets

Added reproducible native/cross macOS ARM64 build and ZIP packaging scripts. Both executables link as ARM64 Mach-O with macOS 11.0 deployment targets and ad-hoc signatures; only system dylibs/frameworks are required. Fixed the vendored sound library AudioToolbox spelling. The SDK is excluded from the archive. See [macOS build notes](MACOS_BUILD.md); no Mac runtime test was available.

A shared text-field renderer draws a cyan end-of-text caret for focused LAN name/IP, object-editor, comparison and map-editor inputs. It reserves space for the caret and handles empty/overflowing Unicode text. Existing input semantics are retained. Native/WASM builds, formatting and lint passed. Comparison and level browser suites passed. Visual checks covered focused, empty and long values in editors and focus switching between native LAN fields. The broader editor browser suite timed out at `shell refines and fractures isolated triangle`; it did not finish, so it is not counted as passing validation. No physics code changed.
