# Classic Tanks / First Battlefield

Open `levels/classic-tanks.json` with **Level Editor → File → Open…** (Ctrl+O), choose Blue, then Play Level. No rebuild is needed to load the JSON.

The supplied screenshot is the layout reference: eight upper brick columns (the two middle columns extend farther), six lower columns, two horizontal cross walls, the central H, three steel obstacles, the fixed outer frame and the bottom brick base enclosure. Battle damage on the central column is restored. The small brick cap over the right steel obstacle is retained as pictured. Thirty-two screenshot pixels map to one metre, preserving the field proportions and providing three-metre lanes.

The Blue tank starts beside the bottom base facing upward. The Red tank starts in the upper-left corridor, facing downward, in **Conquer** mode. The Blue flag stands in for the eagle; a Red flag in the upper corridor supplies Fracture's player victory objective. The base enclosure uses thinner masonry so it is a potential ram target; the large columns remain heavy obstacles. Tanks use the current damage and respawn rules. Enemy activity can be changed in Map Settings.

The level has 30 authored items and 2,596 material cells. Both team spawns were validated, and a five-second simulation checked that the Conquer enemy leaves its starting corridor.

Regenerate the JSON from its source recipe:

```sh
cargo run --locked -p sim --example classic_tanks
```
