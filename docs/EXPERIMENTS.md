**Reproducible comparison range**

**Fence bending:** [open six synchronized trials](http://127.0.0.1:8000/sandbox.html?experiment=fences), or start the native app with `--fences`. **Case 3 is the selected metal fence default: six bend points at 8 kN m.** Case 4 retains the earlier two-bend-point reference. All six now use that same yield moment, with **3 / 4 / 6 / 2 / 8 / 12 bend points** respectively. The fixed support span, outline, mass, friction and impact stay identical; only the free span's resolution varies. Bend points hold their rest angle below yield; permanent deformation requires the actual resisting torque to reach its cap, so solver error alone cannot soften the fence. A tiny elastic allowance (0.002 radians) avoids free hinges. They have effective open-fence density 780 kg/m³ (15.6 kg total), friction 0.02, and minimum contact-friction combination so tank tracks do not snag them. All share a 4500 kg tank pushing at 3 m/s for six seconds. The comparison displays tank turn and sideways offset; case 3 is marked as favorite.

Use **FOCUS CASE**, **Space**, the timeline, and **MARK SELECTED FAVORITE** to compare. **MATERIAL → BENDING TORQUE** changes resistance without rebuilding the application. [The portable study](../web/experiments/fences.json) includes a two-second checkpoint; regenerate it with `cargo run --locked -p sim --example fence_comparison`. The exporter measures clearance time and path deviation until the tank clears the fence, excluding its later collision with the arena wall. The earlier many-section, rotational-brake trials have been replaced.

**Ammunition:** shell trials now save HE, AP, MG or laser in `impact.ammunition`. Missing values default to HE. Choose it from the shared **AMMUNITION** control or the focused **IMPACT → AMMO … >** button. HE/AP fire once; MG/laser fire continuously for the trial duration. Laser hides mass/speed fields, and saved views include its beam, cut geometry and removed area. [AMMUNITION.md](AMMUNITION.md) gives the profiles and cutting rules.

Ground rotational friction is four times stronger for movable objects and debris in `experiment-terrain-bridges-13`. **RUN / RUN ALL** reconstructs saved recipes with the current physics; stored visual checkpoints keep the earlier appearance until rerun. Built-in portable studies have been regenerated.

**Wall release:** [open the new six-case study](http://127.0.0.1:8000/sandbox.html?experiment=barrel-release), or native `--barrel-release`. It addresses the preference that the barrel should separate from the wall. Every case has friction 0.09 and base bounce 0.04; soft-impact bounce varies **0.60 / 0.62 / 0.64 / 0.66 / 0.68 / 0.70**, with soft speed 16 m/s. Case 2 is the selected ordinary barrel default and is marked as favorite. The response smoothly returns to base bounce at 32 m/s. Old reference studies below keep their original constant coefficients.

The selected case-2 trial leaves about **0.620 m** of wall clearance. Its no-backstop recording stays within 1 mm / 0.001 rad of the constant-bounce case-2 reference on the same build. Outcomes depend on impact history: increasing the coefficient need not monotonically increase final separation because the finite blast pulse can cause repeated contacts. **FOCUS CASE → MATERIAL → BOUNCE RESPONSE >** edits the curve and can disable it. Shared shot settings still apply to all six. Save/export/replay retains the full response. `barrel_comparison --release` exports `web/experiments/barrel-release.json`; `barrel_contact --gentle` prints measurements with and without the fixture.


**Barrel shot response:** [open six trials](http://127.0.0.1:8000/sandbox.html?experiment=barrel-shot), **O → L → SHOT RESPONSE / 6**, or native `--barrel-shot`. This replaces the older ten-case button in the object picker; the ten-case URL/native command still work.

| Bounciness | Low friction 0.02 | Current friction 0.09 | Higher friction 0.40 |
| --- | --- | --- | --- |
| 0.04 (current) | Case 1 | Case 2 | Case 3 |
| 0.70 | Case 4 | Case 5 | Case 6 |

Every case uses the same accepted barrel deformation profile: yield 500 Pa, flow 2/s, hardening 0, limit 3. The default shot is 6 kg at 220 m/s, centered, with the fixed backstop 15 cm behind the barrel and a three-second recording. Playback starts looping in slow motion; the shared timeline can be paused and scrubbed. Bounciness affects the barrel's collision with the backstop; the shell is consumed on its first impact. Turning off the backstop lets you compare free sliding, where bounciness has no effect until the barrel collides again.

The shared toolbar edits shell speed, mass, target X/Y, duration and backstop at runtime. **FOCUS CASE → MATERIAL** edits that case's friction and bounciness. Save, favorite, export and replay work as in other studies. The user prefers case 6 with the close backstop and case 2 without it. The two prepared recipes mark those choices separately; case 2 supplies the base restitution and friction used by the new ordinary barrel, which now adds soft-impact bounce. Native results after three seconds put the high-bounce barrel centers at approximately **1.44 / 2.75 / 3.10 m** from an initial X of 3 m; low-bounce cases settle near 3.20 m. These are recipe measurements, not prescribed trajectories.

**No-backstop reference:** [open the preferred case 2](http://127.0.0.1:8000/sandbox.html?experiment=barrel-shot-free), or native `--barrel-shot-free`. It has a separate name, the same six materials, and the backstop disabled. **The arena's outer walls remain active.** With the default shot, the barrel reaches the right wall at roughly 0.3 s, so later motion includes wall rebound and can include further collisions. The recorder now includes these walls; older saved checkpoints need RUN to record their visible geometry. Wall area is excluded from object metrics. No material changes automatically when you toggle the fixture.

Export: `cargo run --release -p sim --example barrel_comparison -- --shot` generates `web/experiments/barrel-shot.json` with a one-second checkpoint and favorite 6. Add `--free` to export `web/experiments/barrel-shot-free.json` with favorite 2. `node tools/barrel-shot-smoke.mjs` verifies the matrix, visible motion differences, exact saved replay, separate object edits, shared shot edits without reloading WASM, editor persistence and high-DPI rendering.


**Six deformation limits:** open [the prepared comparison](http://127.0.0.1:8000/sandbox.html?experiment=crush), or **O → L → DEFORMATION / 6**. Desktop: `cargo run --release -p sandbox -- --crush`. Cases use strain limits **0.4, 0.8, 1.2, 1.6, 2.2 and 3.0**. Geometry, 72 kg barrel mass, yield 0.5 kPa, flow 2/s and hardening 0 remain identical. All six initially use a centered 4500 kg tank at 0.8 m/s, a fixed backstop and eight seconds. The accepted rim-only deformation solver remains unchanged; case 6 (limit 3) is now the ordinary barrel default, selected by the user. No contact-corner constraint was restored.

**TEST ALL CASES** is visible above the six views. Edit speed, approach angle, lateral offset, mass or duration, choose track overlap (center / upper half / lower half), or toggle the backstop. **RUN ALL** recalculates all six using these settings in the running browser/native application. Parameter changes do not require a code build or page reload. Loading the updated application once is sufficient. Each object's material values remain separate. **FOCUS CASE → MATERIAL** changes the selected object's strain limit, yield, flow, hardening and other properties. Six-case comparisons retain six cases when using **MAKE 6 VARIANTS**; shared studies offer object-property sweeps. Factors are 1/3, 2/3, 1, 4/3, 11/6 and 2.5, clamped to supported ranges.

Tank angle is **−180° to +180°**, with 0° traveling right and +90° traveling down on screen. Both starting pose and initial velocity rotate. Offset is perpendicular to the approach; ±0.82 m aligns a track edge to a centered barrel's initial midline. Overlap presets use the selected outline's projected midpoint for custom shapes. The backstop remains on the object's right side; angled approaches that start inside it or another object are rejected with an editable error. Disable the backstop when approaching from behind it. All bodies remain free to rotate and slip after the initial placement. Shell mode exposes speed, mass and target X/Y instead of tank angle/offset.

Shared test settings are saved through `shared_impact: true`, and `impact.angle_degrees` stores the tank angle. Both are optional in older version-1 recipes: missing shared mode preserves independent case impacts, and a missing angle means 0°. The shared toolbar applies changes across every case, and focused impact edits in a shared study do the same. A normal RUN preserves an older experiment's independent impacts; RUN ALL links its cases using the selected impact. Material edits never copy object properties across cases. Validation rejects inconsistent shared recipes.

`cargo run --release -p sim --example barrel_comparison -- --crush` regenerates [the portable six-case recipe](../web/experiments/crush.json) with a six-second checkpoint. `node tools/crush-smoke.mjs` checks six distinct limits, runtime shared settings, actual angled tank poses, six-case sweeps, independent material edits, no page reload while tuning, favorites, replay and high-DPI rendering.

**Ten-barrel crush study:** open [the prepared simulation](http://127.0.0.1:8000/sandbox.html?experiment=barrels). Desktop: `cargo run --release -p sandbox -- --barrels`. It starts and loops ten synchronized four-second trials of a 4500 kg tank driving at 0.8 m/s, with a rigid backstop 15 cm behind each barrel. All barrels retain the same outline, mesh, mass, friction and crush force threshold.

The five columns use effective plastic yield stresses **0.5, 1, 2, 4 and 8 kPa**. The top row uses **hardening 0**; the bottom row uses **hardening 4**. These frames deform permanently and stay connected. Higher yield delays deformation; hardening raises resistance as the object bends. See [PLASTICITY.md](PLASTICITY.md).

Click a numbered case, use **FOCUS CASE** to enlarge it and show its settings, or **MARK SELECTED FAVORITE → SAVE** to remember the choice. **Space** pauses, the shared timeline scrubs, **LOOP** repeats playback, and **T** slows it down. **USE CASE IN OBJECT EDITOR** in focus view copies the selected barrel. The native [export example](../crates/sim/examples/barrel_comparison.rs) regenerates [the portable recipe and review frame](../web/experiments/barrels.json).

The range accepts **1–10 cases**. More than four use a two-row overview; focused mode shows the full inspector. The ordinary sweep command still creates four variants. **IMPACT → PIN AGAINST BACKSTOP** adds/removes the fixture for the selected case, and **COPY IMPACT TO ALL CASES** copies it along with the impact parameters. The optional JSON `impact.backstop` field defaults to false for older experiments. Saved recipes, favorite indices, frames and imports all support ten cases with the existing recording/file size limits.

Open the object editor with **O**, prepare an object, then choose **COMPARE**. The range starts with four copies at 0.5×, 1×, 1.5× and 2× seam strength, or plastic yield for deformable objects. Select a case by clicking its view. **RUN** reconstructs and records all cases; **STOP** ends calculation at the currently available shared time. Running again starts from the recipe's initial conditions.

Use **MATERIAL**, **IMPACT** and **MESH** to edit the selected case. Material controls include density, depth, friction, three seam strengths, grain cohesion, Young's modulus E and fracture-surface work Gc. Impact controls choose a single shell shot or a driven tank ram, with independent shell/tank masses and speeds. **COPY IMPACT TO ALL CASES** sets up a controlled comparison with identical incoming conditions. Duration applies to the whole experiment, from 1 to 10 simulation seconds.

Shell trials fire once from the tank at (-7, 0), targeting the object's local aim point. The object is placed at (3, 0). Tank trials start 0.4 m plus the nominal tank half-length to the left of the object's bounding box, already moving at the chosen speed; both tracks continue to drive. Aim Y selects the tank's offset perpendicular to its approach; aim X only affects shell trials. At nonzero angles, the starting distance uses the object outline projected onto the approach direction. Tank mass rescales its actual colliders and inertia. The arena, shell explosion model, friction solver and timestep come from the recorded engine revision.

Click the sweep selector to cycle through seam strength, stiffness, object density, grain size, fracture work, shell mass/speed and tank mass/speed. **MAKE 4 VARIANTS** replaces the current set using the selected case as its 1× reference. Factors are clamped to supported parameter ranges. For mass/speed sweeps, choose the corresponding impact type first. The material tab shows the actual values; a grain-size sweep regenerates sites from the stored seed. With **OLD MESH**, that sweep instead uses solver detail 0, 1, 2 and 3. Invalid cases show a validation error and must be repaired before running or saving.

**Playback and choosing a result**

| Control | Result |
| --- | --- |
| PLAY / Space | Play or pause the shared recording |
| REWIND | Return to the first recorded frame |
| Timeline | Scrub all cases to the same available simulation time |
| N | Advance one recorded frame (1/30 s) |
| T | Switch between 1× and 0.2× playback |
| Wheel / right-drag | Smooth linked zoom / pan |
| FIT / FOCUS CASE | Fit the shared view / enlarge the selected case |
| Tab / SOLVER MESH | Reveal internal triangles and recent stress coloring |
| MARK FAVORITE | Mark the selected case; SAVE retains the choice |
| USE CASE IN OBJECT EDITOR | Copy the chosen object's shape and material properties into the workshop |
| MESH → EDIT SHAPE / GRAIN POINTS | Edit this case in the workshop; COMPARE returns it to the same experiment |

Each view reports chunks, broken seams, largest surviving chunk area, cleared area, cumulative fracture work and peak virtual elastic energy. Area is more useful than cell count when comparing different meshes. Chunk count includes supported components. The elastic-energy reading comes from the quasistatic diagnostic solve; it is not energy released into debris. A red solver-limit message identifies cases with incomplete stress solves; reduce detail or adjust the mesh before using those results for calibration. Uniform E changes virtual strain/energy, but can leave force-controlled failure patterns unchanged. Destructible chunks remain rigid; deformable frames change their rest shape after yielding. Their inspector replaces failure strengths/Gc with yield, flow, hardening and strain limit, and their cards report plastic strain/work.

**Saving and replaying**

Name the experiment, select a useful time and optional favorite, then **SAVE**. The local library holds up to 12 experiments; saving under an existing name updates that entry. Change the name to keep a separate experiment. **LIBRARY** loads or removes saved entries. Removal deletes only that local entry; exported copies remain available.

Saved version-1 JSON contains every variant's object, explicit stone sites and layout seed, material and impact settings, duration, engine revision, favorite and selected time. It also includes the exact visible geometry, poses and measurements at that time. Loading shows this saved frame immediately. **RUN** reconstructs the full timeline from the recipe. This is a reproducible experiment and visual checkpoint, not a serialized Rapier world or a resumable mid-impact physics state.

**EXPORT / IMPORT** downloads or opens an individual experiment JSON in the browser. Native export/import opens Save As/Open file dialogs; the native library remains `objects/experiments.json`. Browser library storage uses `fracture.experiments.v1` on the current origin. The object draft remains in its separate store. No server or account is involved. Files/library data have an 8 MiB application limit, and browser storage quotas can be smaller; a failed save is reported and leaves the previous library intact. Export provides a portable copy.

Same-build replay equality is tested both natively and in WASM. Cross-platform or cross-version bit equality is not promised. A differing engine revision is shown on load: the saved view remains available, while a new run uses the current physics. Bump `ENGINE_REVISION` when changing simulation or meshing behavior that affects replay.

Physics advances at 120 Hz; geometry is recorded once per cell shape revision (earlier shapes remain immutable) and poses every four ticks. All cases use matching timestamps even when calculation is slower than real time. Calculation receives a roughly 6 ms budget per UI frame; an individual expensive physics step can exceed it. Pose recording stops after approximately two million stored poses (about 64 MB on native, less on WASM, plus geometry, allocator and physics overhead), completing the current shared frame. Shorter experiments and coarser solver detail reduce cost. Save files contain only the selected visual checkpoint, not all playback frames.

See [EDITOR.md](EDITOR.md) for grain authoring and [DESTRUCTION.md](DESTRUCTION.md) for the underlying approximations. Run `node tools/comparison-smoke.mjs` with the browser setup in [SANDBOX.md](SANDBOX.md) to check grain editing, shell/tank comparisons, replay, favorite selection, save/load, file round trips and rendering.

Object-library components and colors travel with each variant and its saved visual geometry. Use the object editor to change behavior components, then return through **COMPARE**. Current engine revision: `experiment-terrain-bridges-13`. Starter profiles need calibration; see [OBJECT_LIBRARY.md](OBJECT_LIBRARY.md).

`node tools/barrel-smoke.mjs` verifies direct launch, all ten visible cards, focus, favorite persistence, backstop configuration, exact WASM replay, selected-profile editing, library launch and high-DPI rendering.
