**A tank combat game built around physical destruction.**

A Rust sandbox with free movement, a dark wireframe battlefield, and destruction driven by forces and broken connections. No health points or textured surfaces. Runs natively and in a browser using Rapier 2D and Macroquad.

Continuing development? Start with the [current development summary](docs/HANDOFF.md). The [contributor guide](CONTRIBUTING.md) maps code locations, extension points and verification commands; [development history](docs/DEVELOPMENT_HISTORY.md) preserves earlier decisions and rejected tuning.

Drive a tracked tank, aim the turret with the mouse, and choose explosive shells, armor-piercing shells, machine-gun bullets or a cutting laser. Projectiles are consumed at impact; the laser progressively removes fine cells without applying force. Structures split along irregular polygon seams; small disconnected fragments fade out while larger sections remain physical.

Build a map with **M / Level Editor**: place library objects, draw long walls, resize rocks, paint seamless terrain, add large bridges and unbreakable walls, and place team spawns and flags. Shift-click or box-select groups, copy/paste with Ctrl C/V, and merge rocks or walls with Ctrl J. Large objects coarsen their mesh automatically; resizing and placement stick to map limits. Playtest as Blue or Red; touching an enemy flag wins. Valid drafts save locally and maps can be exported/imported as JSON. See the [level editor guide](docs/LEVEL_EDITOR.md). **File → Load River Valley** opens the large village-and-river map, with lightweight decorative shrubs, crates and timber that break into visual effects without physical debris.

**Demo map:** open **M → File → Load Demo / Quarry → Play Level**. [Quarry Crossing](docs/DEMO_MAP.md) has a central bridge rush, a sandy northern bypass, a southern road, and editable cover and barrel yards.

**Run on desktop:**

```sh
cargo run --release -p sandbox
```

**Run in a browser:**

```sh
rustup target add wasm32-unknown-unknown
bash tools/build-web.sh
python3 tools/serve.py
```

Open [localhost:8000](http://127.0.0.1:8000). The loader and bold font are bundled locally; no CDN, Node, or JavaScript build step is required. Python only serves the files. Use `python3 tools/serve.py --port 8080` to change the port.

**Try it:** aim at the barrier and click. Use **T** for slow motion and **Tab** to inspect the stress spreading through the structure. Fire again after the reload indicator fills. **R** rebuilds the scene.

| Control | Action |
| --- | --- |
| W / S | Both tracks forward / reverse |
| A / D | Differential steering, including turning in place |
| Q / Z | Override the left track: forward / reverse |
| E / C | Override the right track: forward / reverse |
| Release movement keys | Apply the track brakes |
| Mouse / left click / F | Aim turret / fire / fire at the current aim point |
| 5 / 6 | HE / AP; switching starts a reload |
| B | Cycle ammunition |
| Right click | Spend one collected laser or rocket charge |
| H | Toggle field manual and simulation controls |
| O | Open the object editor / return from testing |
| M | Open the level editor / return from level playtesting |
| Space | Pause or resume |
| N | Pause and advance one physics tick |
| T | Toggle 0.2x slow motion |
| Tab | Show recent connection stress |
| R | Rebuild the scene |
| 1 / 2 / 3 / 4 | Barrier / barrel against rubble / rock field / object yard |
| 9 | Terrain and destructible bridge range |
| Wheel / Home | Zoom / recenter; level camera follows the tank with mouse look-ahead |
| Middle drag | Pan in material test scenes |

The main gun swaps HE/AP rounds with a 0.9-second reload. Rockets and lasers must be collected from editor-placed pickup pads, with configurable charge counts and cooldowns. Rockets travel slowly and shatter rock; the wide laser fires continuously for three seconds and ignores aim spread. See [ammunition controls and behavior](docs/AMMUNITION.md). A shot requested while paused waits for a step or resume. The tracks use front/rear ground contacts, limited engine force, and strong braking. Steering limits the requested track-speed difference, and releasing the controls arrests rotation as well as translation. The turret aims independently of the hull. There is no projectile accumulation limit because shells are consumed on impact.

**River crossing:** press **9**. Road increases tank speed and grip; sand slows it down; water blocks the hull and tracks. Shots pass over the deck; target the booths marked A/B to destroy the crossing. They resist pushing and slow bumps, but remain vulnerable to shooting and forceful direct rams. Losing the connection between its two banks collapses the span, closes the crossing, and sinks a tank standing on it. **R** restores the scene. See [terrain and bridge rules](docs/TERRAIN.md).

Zoom uses normalized mouse-wheel input and time-based easing, independently of pause and slow motion. Editors and material tests anchor zoom at the cursor; level gameplay follows the tank and limits the visible span to twice its sight range.

**Start from the object library:** press **O**, then **L** (or **FILE → OBJECT LIBRARY**). Green barrels, red explosive barrels, amber walls, violet rocks and blue metal fences have distinct editable response profiles. Save named copies, edit color in **SHAPE**, and open **MATERIAL → OBJECT BEHAVIOR** for impact resistance, crushing and explosions. Press **4** in the proving ground for the shared object yard. See [the library guide](docs/OBJECT_LIBRARY.md) for component units, persistence and the current physical approximations.

**Permanent dents and bends:** barrels flatten and bulge sideways under compression, retaining their dents after the tank releases them. The blue metal fence bends while its pinned edge stays attached. Choose **MATERIAL → YIELD & HARDENING**, or switch any object through **OBJECT BEHAVIOR → PERMANENT DEFORMATION**. Deformable frames stay connected under impact; the laser can cut them. Rocks and walls still fracture. See [the deformation guide](docs/PLASTICITY.md) for controls, replay and physical limits.

**Build your own objects:** press **O**, or click **EDITOR**. Use **FILE** to start from a rock, wall, plate, or concave L-block. **SHAPE**, **FRACTURE**, and **MATERIAL** group the sidebar tools; advanced settings expand when needed. Drag outline vertices, Shift-click an edge to insert a vertex, or enter exact X/Y coordinates. Edit density, thickness, friction, bounciness, tensile/compressive/shear seam strengths, fracture detail, and grain cohesion. Select a vertex and pin its outgoing edge to add a breakable fixed support. The preview shows the mesh, area, and mass.

**Enter / TEST** creates the object in the firing range. **R** rebuilds it; **O** returns to the unchanged draft. Undo/redo includes shape edits, property edits, presets, and imports. Valid drafts save automatically. Browser **FILE → EXPORT / IMPORT JSON** downloads and opens versioned JSON files; on desktop these buttons open native Save As/Open dialogs for the chosen JSON file. Desktop drafts live in `objects/draft.json`; browser drafts live in local storage. See [the editor guide](docs/EDITOR.md) for controls, property units, and current limits.

**Shape the fractures:** open **FRACTURE** and choose **STONES** or **BRICKS**. Adjust grain width/height, drag stone grain points, Shift-click to add a point, or Delete to remove one. Bright grain seams are separate from the internal solver triangles. E and Gc are now editable too.

**Compare ten barrels:** open [the prepared crush simulation](http://127.0.0.1:8000/?experiment=barrels) while the server is running. Ten synchronized tank/backstop trials span different plastic yield stress and hardening. Click a case, focus it, and mark/save your favorite. Desktop: `cargo run --release -p sandbox -- --barrels`.

**Updated obstacles:** the library Wall has weaker masonry seams and a single breakable support: a standard shell produces rubble, while a tank ram separates larger blocks. Rock uses the former wall’s seam strengths and a central ground attachment. Load either through **O → L**; **SHAPE** exposes the ground anchor and individual edge pins.

**Less spinning:** ground rotational friction is four times stronger for objects and debris. Reload the browser build and use **RUN ALL** to see it in saved comparisons.

**Barrel wall release:** [open the new six-case trial](http://127.0.0.1:8000/?experiment=barrel-release). Case 2 is the selected barrel default: it separates from the close wall while matching the constant-bounce case 2's no-backstop motion within 1 mm on the same build. The material uses stronger bounce for softer collisions and low bounce for hard impacts; friction remains 0.09. Edit **MATERIAL → BOUNCE >** in the editor, or **FOCUS CASE → MATERIAL → BOUNCE RESPONSE >** in comparisons. Desktop: `cargo run --release -p sandbox -- --barrel-release`.

**Compare barrel bounce and friction:** open [the six shell-impact trials](http://127.0.0.1:8000/?experiment=barrel-shot), or **O → L → SHOT RESPONSE / 6**. Columns use friction **0.02 / 0.09 / 0.40**; rows use bounciness **0.04 / 0.70**. All use the accepted barrel profile (strain limit 3), the same 6 kg / 220 m/s shell and a backstop. Playback loops in slow motion. Edit the shared shot controls then **RUN ALL**, or focus a case to edit **MATERIAL → FRICTION / BOUNCINESS**. Desktop: `cargo run --release -p sandbox -- --barrel-shot`.

The selected shot references are **case 6 with the backstop** and **[case 2 without it](http://127.0.0.1:8000/?experiment=barrel-shot-free)**. Each link remembers its own favorite. Without the close fixture, the barrel can still strike the outer arena walls, now visible in recorded comparisons.

**Compare deformation:** open [the six-case simulation](http://127.0.0.1:8000/?experiment=crush), or **O → L → DEFORMATION / 6**. Six barrels use strain limits **0.4 / 0.8 / 1.2 / 1.6 / 2.2 / 3.0** under identical impacts. The **TEST ALL CASES** bar edits speed, approach angle, offset, tank mass, duration and backstop while all six views stay visible. Choose **RUN ALL** to recalculate in the running app—no compilation or page reload. **FOCUS CASE → MATERIAL** edits one object's properties; **MAKE 6 VARIANTS** creates another material sweep. Mark/save your favorite and export/import experiments as usual. Desktop: `cargo run --release -p sandbox -- --crush`.

**Compare destruction:** choose **COMPARE** to run four synchronized shell or tank trials. Sweep material, grain size, mass or speed; scrub the shared timeline; mark a favorite and bring it back to the editor. **SAVE** stores the reproducible recipe and selected visual frame. **LIBRARY** reloads trials; **EXPORT / IMPORT** shares them as JSON. See [the comparison guide](docs/EXPERIMENTS.md).

**Simulation:** `crates/sim` advances at 120 Hz, with four physics substeps per tick. Each connected chunk is one Rapier rigid body, regardless of its internal cell count. A separate elastic interface graph estimates tension, compression, shear, and bending. Overloaded seams can break if the impact supplies enough fracture work; stress is recalculated after each crack. Splitting preserves mass and the parent's rigid velocity field.

Strong impacts locally subdivide large triangles, including cells inside whole objects and previously detached chunks. New internal seams start intact. This gives later shots somewhere to crack a large triangle without making finely detailed rocks flexible. Disconnected chunks below 0.075 m² fade after roughly 3–4 simulation seconds. Press **H** to see chunk/cell counts and refinement limits.

Destructible objects use a rigid-chunk brittle-fracture approximation. Its stress graph is quasistatic, its material constants are illustrative, and its crack-work budget is not a full cohesive traction/separation or dynamic FEM model. Crack paths still follow triangular edges. Barrels and metal fences use a separate permanent deformation mode; armor penetration, general vertical physics, sound and multiplayer are still deferred. Bridges use an explicit 2.5D collapse rule. High initial detail still increases collision and fracture costs; start coarse and let impacts add local detail. See [the simulation notes](docs/SANDBOX.md) for implementation, measured limits, and validation.

**Validate:**

```sh
bash tools/check.sh                 # format, tests, clippy
bash tools/check.sh --web           # also build the browser version
cargo run --release -p sim --example measure
cargo run --release -p sim --example impact
cargo run --release -p sim --example resolution
```

`bash tools/check-browser.sh` runs all browser suites sequentially; append suite names to select them (for example `editor comparison terrain`). `node tools/browser-smoke.mjs` runs the gameplay interaction checks and saves screenshots to `target/screenshots/`. It needs Node 22+, the local server, and an isolated Chrome instance with remote debugging on port 9227. See [the browser verification instructions](docs/SANDBOX.md).

`node tools/editor-smoke.mjs` checks authoring, physics testing, undo/redo, file import/export, draft restoration, invalid input, and high-DPI rendering in the same browser setup.

`node tools/comparison-smoke.mjs` checks editable grains, synchronized shell/tank trials, exact replay, saved checkpoints, library persistence and experiment file round trips.

`node tools/library-smoke.mjs` checks the original four starters, component/color editing, saved copies, reload, invalid-save preservation, explosions and yard navigation.

`node tools/plasticity-smoke.mjs` checks deformation settings, undo/save/reload, real tank/fence contact, permanent bends and reset. `node tools/barrel-smoke.mjs` verifies ten deformable barrels and exact shape replay.

`node tools/crush-smoke.mjs` verifies six deformation limits, shared runtime impact settings (including actual approach angles), separate material edits, saved favorites and exact replay.

[The original project plan](docs/PLAN.md) covers the engine comparison, architecture, and later milestones.

[The destruction architecture](docs/DESTRUCTION.md) explains rigid chunks, stress transmission, adaptive refinement, and the remaining physics limitations.

`node tools/barrel-shot-smoke.mjs` checks friction/bounce variants, rebound travel, exact replay, runtime shot/material edits, editor persistence and the library shortcut.
