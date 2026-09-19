**Object workshop**

Open with **O** or the **EDITOR** button. This workshop authors one destructible object at a time: rocks, walls, plates, and custom concave polygons. A test places that object beside the tank. Machine assembly and custom scene placement are later work. **L / FILE → OBJECT LIBRARY** opens barrel, explosive barrel, wall and rock starters plus your saved copies. See [the library guide](OBJECT_LIBRARY.md).

The sidebar has three task tabs: **SHAPE** for outline vertices and supports, **FRACTURE** for grain layouts and grain points, and **MATERIAL** for physical properties. Only the selected tab's tools appear. **FILE** holds shape presets and JSON import/export; **VIEW** holds snapping, solver-mesh visibility and fit. **TEST** and **COMPARE** stay in the toolbar. Escape or an outside click dismisses a dropdown without acting on the canvas.

**SHAPE → COLOR** sets an object color (`#RRGGBB` or `AUTO`). **MATERIAL → OBJECT BEHAVIOR** opens a dedicated page for shell/contact resistance, opposed-contact crushing, explosions and contact refinement. Components apply to any shape and are preserved in JSON and comparison recipes.

**MATERIAL → FAILURE & STIFFNESS** expands the strength, E and Gc fields. **FRACTURE → SOLVER DETAIL** expands the refinement control. Both start collapsed. Stone point handles appear in Fracture; outline handles appear in Shape. Material is a preview view, so Delete there cannot remove geometry.

| Action | Control |
| --- | --- |
| Select / move an outline vertex | Left click / drag its circular handle |
| Insert a vertex | Shift-click near an edge, or **SHAPE → ADD** at the selected edge's midpoint |
| Remove selected vertex | **SHAPE → REMOVE** or Delete |
| Enter an exact coordinate or property | Click the value, type, then Enter or Tab; Escape cancels |
| Select the entire field | Ctrl/Cmd+A while editing |
| Snap dragging to 0.1 m | **VIEW → SNAP TO 0.1 M** |
| Pin an edge | Select its first vertex, then **PIN SELECTED EDGE** |
| Undo / redo | Toolbar, Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl/Cmd+Y |
| Zoom / pan / fit object | Wheel / right drag / Home |
| Choose fracture layout | **FRACTURE → STONES**, **BRICKS**, or **LEGACY** |
| Edit stone grains | **FRACTURE → STONES**, then drag, Shift-click to add, Delete to remove |
| Edit the object's boundary | **SHAPE** |
| Preview solver triangulation | **VIEW → SHOW SOLVER MESH** (off initially) |
| Compare reproducible trials | **COMPARE** |
| Test in firing range | **TEST** or Enter outside a text field |
| Return to editor from testing | O |
| Rebuild test object | R in the firing range |
| Export JSON | **FILE → EXPORT JSON** or Ctrl/Cmd+S outside a text field |
| Return to the previous simulation | **BACK**, O, or Escape outside a text field |

The selected vertex's outgoing edge is white; its next vertex follows the numbered outline. A pinned edge is violet. Inserting a vertex on the pinned edge or deleting a vertex clears the affected support so its meaning stays explicit. Pinning creates a fixed support. Its seams can break for destructible objects; deformable frames keep the pinned edge fixed while bending.

| Property | Meaning |
| --- | --- |
| Density, kg/m³ | Mass per unit volume; 100–20,000 |
| Thickness, m | Out-of-plane depth; 0.02–2.0 |
| Friction | Collider friction and approximate resistance against the horizontal floor; 0–2 |
| Bounciness | Collision restitution, 0–1. Larger values increase rebound from other objects. Contact coefficients are averaged between both surfaces. Old files default to 0.04. |
| Tension / compression / shear, kPa | Seam failure stress; 1–1,000,000 each |
| Grain cohesion, multiplier | Strength and fracture-work multiplier inside a coarse grain compared with its outer seams; 1–100 |
| Young E, GPa | Virtual elastic stiffness; 0.0001–1,000 |
| Gc, J/m² | Work required per unit new fracture surface; 0.1–100,000 |
| W / H, m | Target grain width/height, 0.15–4; changing these regenerates the layout |
| Seed | Integer 0–4,294,967,295; regenerates stone sites deterministically |
| Fracture detail, 0–4 | Initial internal detail: each increment subdivides every triangle into four cells |

Material presets reset density, E, Gc and seam strengths to the existing sandbox values and select a rendering palette. They leave thickness, friction, bounciness, geometry, and fracture settings intact. These are game-tuned seam properties rather than measured engineering material data. Nothing uses health points.

Fresh presets use compact stone grains (rock/L-block) or staggered bricks (wall/plate), with solver detail 0 and cohesion 12. **STONES** partitions the outline into Voronoi regions around editable points. The **FRACTURE** tab exposes their handles; the X/Y fields, add/delete controls, snapping and undo act on the selected point. Points may sit outside the outline but must remain in workshop bounds and at least 4 cm apart. Their cells are clipped to the outline. **REGENERATE POINTS** replaces manual point edits using the current dimensions and seed; undo restores them.

**BRICKS** fits staggered rows to the outline's bounding box. W/H specify maximum target dimensions, with integer rows/columns and half-bricks at alternating ends. Seed does not affect bricks. **LEGACY** preserves the earlier ear-clipped triangular grains for existing assets and regression comparisons. Existing saved drafts retain their original layout until changed explicitly.

Both generated modes support concave outlines. Shared faces shorter than 3 cm are collapsed consistently across neighbors where the outer boundary can be preserved. Grain interiors are triangulated compactly; the mesher checks area, orientation, joined edges and thin cells. Extreme authored configurations can still be invalid and require a moved point, different size or seed. Grain boundaries are bright violet; internal solver triangles are dim. Detail subdivides those internal triangles without changing the grain layout. In simulation, intact internal edges are hidden; cracks reveal them. Grain cohesion controls how readily a grain breaks internally, so compact grains encourage chunkier debris without guaranteeing a fragment size.

The preview is the initial simulation mesh. Collision and rendering use the exact same triangles, with no seam clearance or density compensation. Total mass equals authored area × thickness × density.

All connected cells share **one rigid body**. More detail supplies more possible crack paths; it no longer adds soft joints or makes a rock rubbery. A pinned object stays fixed until its support connections fail. Detached connected sections become separate rigid bodies with their own mass, inertia, and inherited motion.

Strong impacts can subdivide a large cell at runtime, even inside an otherwise intact object. This also lets subsequent hits break a large isolated triangle into smaller pieces. Each subdivision adds four bonded children, conserves geometry and mass, and preserves existing cracks. Higher cohesion encourages larger chunks; it does not guarantee their size or count. Changing the outline clips the stored sites to its new boundary; regenerate to redistribute sites. E and Gc are editable and persist with the asset. Uniform E changes virtual strain/energy and does not necessarily change force-controlled failure in this quasistatic model. See [the destruction model](DESTRUCTION.md) for values and approximations.

The editor allows at most **2,048 initial cells**, **128 outline vertices**, **128 grains**, and **64 KiB per JSON file**. These are authoring guardrails, not measured performance ceilings. Detailed intact objects still have more colliders and larger stress systems. Start at detail 0–1 for generated grains and let impacts add local detail. Runtime refinement is limited to eight generations and 8,192 total retained cell records per scene, including retired parents and cleared debris; reset reclaims this budget. The field manual reports when a refinement or solver limit is reached.

Invalid crossings, overlapping edges, excessively thin cells, unsupported versions, and invalid material values disable testing and export. Outlines must fit within ±4.5 m X and ±5.5 m Y. Holes and disconnected outlines are not supported yet. Saved version-1 JSON remains compatible; runtime cracks and subdivisions do not overwrite the authored draft.

Valid drafts save after edits and at the end of a drag. Invalid edits remain repairable in the current session but do not replace the last saved valid draft. Undo retains up to 64 edits in memory. Testing and returning preserve the draft and history; a page reload restores the last saved valid object without history.

In the browser, drafts use this origin's local storage and JSON export produces a download named after the object. Import opens a file chooser and validates the complete file before replacing the draft. On desktop, draft storage is `objects/draft.json` relative to the working directory. Explicit export/import opens native Save As/Open dialogs so you can choose any JSON file and folder. The last selected folder is remembered; cancelling leaves the draft unchanged. Draft working files are ignored by Git. No server or account is involved.

The portable definition lives in `crates/sim/src/object.rs`. `ObjectAsset::from_json` validates an asset; `World::from_object` builds its physical representation. JSON version 1 stores SI units, including strengths in **Pa** (the UI displays kPa). Browser file/storage glue lives in `web/editor.js`, with a bounded WASM import buffer. The editor uses the same Rust UI and physics on desktop and in the browser.

For synchronized shell/tank trials, parameter sweeps, a favorite selection and portable saved recipes, see [the comparison guide](EXPERIMENTS.md).

**Deformable frames:** barrels and the metal fence expose **MATERIAL → YIELD & HARDENING**. Any object can switch between **DEFORMABLE** and **DESTRUCTIBLE** through **OBJECT BEHAVIOR → PERMANENT DEFORMATION**. Deformable frames use the authored mesh, permanently update both triangles and colliders, and retain their mass. They do not fracture or acquire runtime subdivisions. Yield/flow/hardening/strain limit replace fracture controls in this mode; undo, drafts, saved library copies and exports preserve them. The simulation never writes its dents back into the draft. See [PLASTICITY.md](PLASTICITY.md).


**MATERIAL → BOUNCE >** opens contact-response controls. Friction controls floor/contact sliding; base bounce is the constant coefficient when soft-impact bounce is disabled. Enable **SOFT IMPACT BOUNCE** to set its coefficient and speed (0.1–100 m/s). The coefficient returns smoothly to base at twice that speed. Contact pages, object JSON, undo/redo and drafts all retain the curve. Comparison MATERIAL has the matching **BOUNCE RESPONSE >** page.


**Ground and edge attachments:** SHAPE has a **GROUND ANCHOR** toggle with a center marker and region guide. Central stone grains form a buried base; disconnected chips become movable. **PIN SELECTED EDGE** can now retain multiple supported edges. Toggle an individual edge to release only that support. Inserting a vertex removes the support on the edited edge and remaps the other indices; deleting an outline vertex clears edge supports. Ground attachment follows the edited shape. Both forms of attachment survive undo, saved objects and experiment replay. The updated Wall starter has two end supports; Rock has a central ground attachment.

Barrel outlines are constrained to a circle. Dragging a rim point or editing its coordinates adjusts the radius; inserting and deleting rim points is disabled. This does not restrict plastic deformation during simulation. Large instances placed in the level editor use automatic grain coarsening; the object workshop retains its physical editing bounds.
