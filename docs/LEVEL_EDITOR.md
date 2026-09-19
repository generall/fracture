# Level editor

Press **M**, or click **M LEVEL EDITOR** in the proving ground. The map editor creates independent recipes that can be played, restarted, saved and shared. Testing never changes the authored map. The first draft is a 48 × 30 metre arena with a spawn and a flag for each team.

For a ready-to-play layout, choose **File → Load Demo / Quarry**. [Quarry Crossing](DEMO_MAP.md) includes three routes, varied terrain, cover and barrel yards. The demo loads as an editable copy and can be undone.

## Place and shape

**Fog of war is enabled during level playtests.** Press **V** or click **Fog of War** in the bottom bar to toggle it. The turret points a 90° vision cone out to 24 m, supplemented by a 3 m nearby circle. Your allied flag also provides a permanent 3 m circle of vision, including while your tank respawns; it follows the same sight-blocking rules and object-memory behavior. Walls, rocks and solid cover block both; erased openings and broken walls let sight through. Barrels and fences transmit sight. Select an ordinary object and use **Sight / Blocks** or **Sight / See Through** to override its default without changing collisions.

Terrain is always known. The enemy flag’s starting position is also known from the start, shown as a dim remembered marker until it enters sight. Seeing any part of an object reveals its **whole figure**, including a tank's gun and health bar. Intact walls and rocks are single figures; detached rubble is observed separately. This reveal does not extend sight to other objects behind cover. Objects start hidden. After leaving sight, a figure stays visible and keeps moving for one second to prevent flicker; seeing it again resets this grace period. When a visible figure splits, its fragments inherit the remaining grace period and replace the parent image. Once the grace period ends, it retains a dim image of its last displayed state: hidden movement, damage, destruction and enemy health do not update it. Looking back refreshes the figure, including clearing a remembered object from a now-empty position. Panning and zooming preserve this map-space memory. Turning fog off does not fill the memory with unseen objects; restarting or starting a new playtest clears memory. Shells, explosions and sparks appear only where currently visible and leave no remembered images.

Fog selects current observations and saved object states for the normal world renderer. Both use the same vector geometry, line widths and camera scale; memories are drawn dimly. There is no separate fog renderer, object texture, or pixel mask. Effects use the same renderer and are shown when their location is visible.

**Tall Grass** is available under **Terrain**. It blocks sight but allows movement and shooting, with ordinary ground traction. Inside grass, visibility is limited to 1.2 m. Paint ground over it to clear it. Its location remains visible as part of the terrain map. Enemy visual targeting also respects grass; physical weapon clearance remains a separate check.

- **Objects:** built-in barrels, explosive barrels, walls, rocks and fences, followed by your saved object-library copies. Select an entry and click on the map. Placed copies keep their material, deformation, explosion, friction and anchoring settings; editing a copy never changes the library asset.
- **Eraser:** next to **Select / Move**. Click to erase one 1 m cell, or drag a rectangle of cells; the red preview shows the cut applied on release. Cuts affect ordinary objects and unbreakable walls, including overlapping layers. Terrain, bridges and team markers are untouched. Cuts persist through save/load, moving, rotating, resizing and copying. Fully erased objects are removed. **Ctrl+Z** undoes a stroke; **Esc** cancels a drag. Objects with erased sections stay separate when using auto-join or merge, preserving their holes.
- **Draw Wall / Draw Unbreakable Wall:** both tools are together under **Objects**. Click to fill one 1 m grid cell, or drag to fill a rectangle of whole cells, including the starting and ending cells. Walls follow the grid axes; use the inspector to rotate or reshape them afterwards. Each gesture creates one instance, with brick grains for destructible walls. Unbreakable walls block tanks and weapons and never fracture.
- **Terrain:** click or drag to fill grid cells with ground, road, sand or water. Touching and overlapping tiles render as one seamless area, with no internal borders or doubled fills; the drawing grid remains visible while a drawing tool is active. Sand, water and bridges have no on-map titles. The most recently placed ground/road/sand patch wins an overlap. Water takes precedence over all of them; an intact bridge provides the crossing. Delete or resize a water patch to remove water.
- **Bridge + Two Booths:** fill a grid-aligned deck rectangle, with a minimum of 3 cells along the span and 2 across the lane. Its long side initially sets the crossing direction; the inspector can switch direction. Drawing reserves room inside the map for the two projecting booths. Before testing, both booths must clear the water. The deck is not a weapon target; the booths retain the accepted reinforced behavior and can be destroyed. Collapse closes the crossing and sinks an occupying tank.
- **Special:** use **Unbreakable Block** to fill a rectangle of cells with a fixed obstacle. Click to place a Blue/Red spawn or flag. Placing another marker of the same kind/team moves the existing marker.

Drawing tools show a filled preview and always use the 1 m grid, at every zoom level. They keep complete cells inside the map border; bridges also reserve booth clearance. A click or drag is one undo step. Library objects and team markers retain point placement.

Regular masonry walls create no automatic footing blocks in level play, including walls saved with the earlier footing settings. Place **Draw Unbreakable Wall** or **Unbreakable Block** alongside or inside a wall section to support it. Embedded and partially overlapping blocks carve their footprint out of the wall, then bond along the exposed cut edges, including rotated shapes. The wall and support never occupy the same volume. Facing parallel edges also attach when they touch or have a gap of at most **0.25 m** and overlap along an edge; corner-only contact does not attach. These connections use the wall material and can fracture, releasing the masonry while the manually placed support remains. Connections are rebuilt on play/restart; loose debris does not attach itself during play. A wall without a nearby support is free-standing. Object-workshop fixtures and bridge booths keep their own support rules.

**Object overlaps:** unbreakable objects take priority over destructible objects; the most recently placed item wins within either group. Lower-priority material is carved away in both the editor and simulation, including fully enclosed cutouts. Selection follows the remaining material. Cuts update when objects move, rotate, resize or disappear; the original editable outlines stay in the saved recipe, so moving a cutter restores the material it covered. Terrain, bridges and team markers retain their separate rules.

Use **Select / Move** to pick and drag an item. The inspector offers position, width, height and rotation where appropriate. Drag the cyan corner to resize, or type exact dimensions and press Enter. Objects with a grain layout also expose grain width and height. The editor coarsens grains automatically as an object grows. Oversized dimensions and drags clamp to the map boundary instead of undoing your gesture. The complete footprint, including bridge booths, stays inside the arena.

**Barrels stay circular:** change their diameter or drag the corner. In the object workshop, dragging a rim point changes the entire radius; inserting and deleting rim points is disabled. This authoring constraint does not restrict deformation during gameplay.

**Edit Outline Vertices** exposes vertices on destructible objects and fixed obstacles. Drag vertices to reshape, or Shift-click an edge to add a point (up to 128). Adding a point clears edge supports, matching the object editor’s topology-edit behavior. Existing vertices preserve support indices. Outlines must remain simple and nondegenerate. Invalid changes are rejected and the previous valid recipe remains available.

Dimensions/outline changes regenerate the grain layout at its existing physical grain size while it fits the budget, then increase spacing automatically. This avoids stretching wall bricks into long slivers. Custom grain-site edits within a library asset are preserved on placement, but regenerated when that instance is reshaped. Grain regeneration can change fracture paths.

Mouse wheel zooms smoothly around the pointer. Right-drag pans; Home fits the map. Optional **Move Snap** uses 0.25 m increments for library/marker placement and selection drags; drawing always fills 1 m cells. Delete removes the selection; Duplicate copies it with a small offset. Undo/Redo and Ctrl-Z/Ctrl-Y cover authored changes, including imports and a new map. Escape cancels a drag or returns to selection. Numeric fields use Enter to apply and Escape to cancel.

## Select, copy and combine

- **Shift-click** adds or removes items. Drag empty space to box-select; Shift-drag over terrain can start a selection box. **Ctrl A** selects all. Drag any selected item to move the group, or edit its centre X/Y. The whole group sticks to map borders without changing relative spacing.
- **Ctrl C / V** copies and pastes, **Ctrl X** cuts, **Ctrl D** duplicates. The clipboard survives map changes within the editor session. A group from a larger map scales down to fit. Team markers can be cut and pasted, but cannot be duplicated while that team's marker already exists.
- **Auto Join** combines touching or overlapping rocks and walls with matching physical properties after placement or movement. Nearby axis-aligned wall edges snap together. Hold **Alt** to suppress snapping/joining, or turn Auto Join off.
- **Ctrl J / Merge Shapes** explicitly combines selected compatible objects. The result is one polygon with a regenerated shared fracture mesh, without duplicate material in the overlap. Unbreakable walls can also merge. Barrels and plastic fences remain separate. Merges that enclose holes are rejected, since the current object schema supports simple outlines only.
- Placement plus automatic joining, explicit merge, group transforms and paste each occupy one undo step. Merging retains supports on surviving boundary edges and the primary object's ground-anchoring setting.

## Play and flags

Choose **Team Blue** or **Team Red** in the toolbar, then **Play Level**. The simulation creates your controllable tank at that team’s spawn and an AI tank at the opposing spawn. A map with only your spawn remains a solo test. A missing spawn, an overlapping tank footprint, a spawn in open water or unsupported bridge booths prevents testing with an explanatory message.

A living enemy tank touching the circular flag base with its actual hull or tracks captures it and immediately wins. Friendly contact has no effect. Flags have no physical collider, so HE/AP/MG/laser, blasts, debris and other objects cannot damage or capture them. The winning result is emitted once and the simulation stops. No carrying the flag home is required.

**R** restarts the authored level; **M** returns to the editor. Either team can be tested locally. The two tanks are local simulation actors; networking is not implemented.

## Enemy activity and tank damage

Click **Map Settings** (or clear the selection) to choose the opponent's behavior:

1. **Static:** remains in place and never fires.
2. **Driving:** follows a deterministic patrol route and never fires. Routes avoid unbreakable geometry, open water and connected breakable chunks of at least 4.5 tonnes. Lighter breakables are traversable at a higher route cost, so the tank may attempt to ram them. Routes update as the scene changes.
3. **Attack:** pursues your tank. It samples your position every 0.35 s, needs 0.7 s of continuous sight before firing, turns its turret at about 60°/s, and fires AP at most every 1.6 s. Aim error is up to 2° stopped or 4° moving. Cover blocks shots. It tries to stop at firing distance.
4. **Conquer:** routes toward your flag, engaging your tank within 12 m. Without a player flag it pursues the player instead.

Attack and Conquer shoot breakable obstacles blocking their path, using HE when there is enough distance and AP up close. They stop to aim, try to back away when too close, then continue through the opening. They prefer going around massive objects, but can attempt to breach them if no route reaches the goal. Unbreakable obstacles remain impassable. A visible player takes priority over clearing cover.

The activity applies to whichever team is opposing the selected player team. Existing maps default to Static. The mode saves, exports and participates in undo/redo with the map.

Both tanks have **100 hull health** and directional armor: strongest at the front, weaker on the sides, weakest at the rear. Health bars appear above tanks. AP penetrates armor, HE relies on its nearby blast, ordinary MG fire does not penetrate, and sustained laser exposure damages the hull. Cover and distance reduce blast damage. Hard ramming impacts also damage the hull in proportion to impact energy; gentle contact and steady pushing are harmless. This is a first gameplay tuning; destructible scenery continues using material stress and fracture, not health. See [tank combat](TANK_COMBAT.md) for the damage rules and suggested extensions.

Destroyed tanks disappear and respawn after **3 seconds**, at their own spawn or the nearest tested clear position within 6 m. An occupied area delays respawn instead of creating overlapping colliders. Respawn restores hull health and grants 1 second of damage protection (a ring around the tank). A bridge collapse can destroy either tank. Destruction of scenery persists across respawns; **R** resets the entire level. Capturing a flag still ends the match immediately.

## Save and share

Valid edits automatically save the current draft. **File → Save As… (Ctrl+S)** lets you choose a file name and folder on desktop; **Open… (Ctrl+O)** lets you choose an existing JSON map. The dialog remembers the last folder, confirms overwrites and leaves the map unchanged when cancelled. Save Draft and Reload Draft are separate recovery actions. New Empty Level and Load Demo / Quarry remain in the same menu.

- Browser draft: `fracture.level.v1` in local storage. Export downloads a named JSON file; Import opens a file picker.
- Native draft: `levels/draft.json`, written via a temporary file and rename. Explicit Save As/Open use native file dialogs; no fixed export/import path is required.
- An unreadable original draft is preserved. The editor reports the error and blocks overwriting it; exporting the working draft remains available.

Version-1 level JSON embeds object recipes, authored transforms, terrain rectangles, bridges, spawns, flags and the optional `enemy_activity` setting (`static`, `driving`, `attack`, `conquer`). It is an initial recipe, not a running Rapier state/checkpoint. The optional object `circular` field preserves the barrel authoring constraint; legacy named barrel recipes remain recognized.

Authoring guardrails: 12–200 m per map dimension, 128 items, eight bridges, 8,192 initial piece records, 2 MiB JSON. Objects allow 128 outline vertices, 128 grains and 2,048 mesh cells. Large level instances automatically target at most 96 grain regions, with fewer regions when uniform refinement is enabled. Dimensions clamp at 190 m or the available map space. Bridges allow spans 3–190 m and lane widths 2–190 m; their dedicated grid uses at most 256 triangles, including 16 booth triangles, regardless of span. These are guardrails, not a frame-rate guarantee for fully detailed maps.

## Code and checks

- `sim/level.rs`: portable schema, validation, instance geometry and bridge grid.
- `sim/level/authoring.rs`: constrained transforms, polygon unions and support preservation.
- `sandbox/terrain_view.rs`: cached terrain composition shared by editor and game.
- `sim/world/level.rs`: runtime loading, collision/spawn checks and flag capture.
- `sandbox/level_editor.rs` and `level_editor/{canvas,panels,fields,selection}.rs`: draft/history, placement, inspector and text entry.
- `sandbox/level_storage.rs` and `web/levels.js`: native/browser persistence.

Run `bash tools/check.sh --web` and `bash tools/check-browser.sh level level-improvements`. The new simulation tests cover large walls, independent asset copies, JSON validation, spawn clearance, fixed obstacles, bridge orientations and weapon-immune tank-only flags. The browser suites exercise multi-selection, clipboard actions, unions, boundary and size clamping, large bridges, actual editor gestures, saved library copies, persistence, both teams’ victories and high-DPI rendering.
