# Quarry Crossing

A 48 × 30 m demo map with 31 authored objects plus a half-metre terrain mask. Open **M → File → Load Demo / Quarry**, choose Blue or Red, then **Play Level**. Loading the demo is undoable. All placed objects and terrain remain editable.

The same map is available as [portable level JSON](../web/levels/quarry-crossing.json), served in the browser at `/levels/quarry-crossing.json`. The bundled editor action and downloadable file use this single source; object recipes are embedded so future library tuning does not silently change the map.

## Layout

- **Centre:** the fastest flag-to-flag road, through masonry gates and a destructible bridge. The bridge booths can be shot from either starting bank. Destroying one closes the crossing.
- **North:** a broad sandy bypass around the water. Grounded rocks and deformable fences make a quarry area beside this slower approach.
- **South:** a longer road bypass. Low masonry walls and explosive barrels provide targets and cover beside the route.
- **Bases:** opposite team flags and clear spawns, with fixed shelter blocks and ordinary barrels that can be crushed against them. Both teams have the same routes and comparable cover.

The main routes have tank clearance without first destroying anything. Cover sits beside the lanes so players can experiment with shooting, squeezing barrels and bending fences. Both bypasses remain usable after bridge destruction; the bridge is a shortcut rather than the only way to finish.

Use WASD or the independent track controls to drive, mouse to aim/fire, and 5–8 for ammunition. R restarts the map and M returns to its editor recipe. As with other level tests, there is currently one controllable tank; testing the opposite team uses its spawn and flag objective, without adding an AI opponent.

## Validation

Simulation checks drive both teams across the bridge and drive both bypasses for each team after destroying a booth with HE shells. They also check for overlapping object colliders and verify that the map remains undamaged and stationary at rest, with fewer than 1,600 material cells. The browser demo suite loads through the actual menu, checks undo/redo and the downloadable JSON, and drives both teams to victory.

To update the map, edit it in the level editor, export it, and replace `web/levels/quarry-crossing.json`. Run `cargo test --locked -p sim world::level::demo_tests`, `bash tools/check.sh --web`, and `bash tools/check-browser.sh demo level`.
