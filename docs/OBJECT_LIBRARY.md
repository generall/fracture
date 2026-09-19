**Object library — first playable profiles**

Open the editor with **O**, then **L** or **FILE → OBJECT LIBRARY**. Choose a starter, edit its shape/material/components, and use **TEST** or **COMPARE**. The five starters are permanent recipes. **SAVE CURRENT** stores an editable copy under the draft's name; the same name updates that copy. Rename in **SHAPE** to keep another variant. **SAVED COPIES** loads or removes copies.

| Starter | Color | Initial behavior |
| --- | --- | --- |
| Barrel | Green, `#69E0AE` | 1.3 m diameter, approximately 72 kg; low friction, strong shell resistance. Opposed tank/backstop contacts enable crushing. |
| Explosive barrel | Red, `#FF6C60` | Same round body and mobility, lower shell resistance. First permanent deformation ignites one 600 kJ, 3 m blast. |
| Wall | Amber, `#F3C66E` | Heavy masonry with compact bricks and a single breakable support. A standard shell produces rubble; a tank ram separates larger blocks. |
| Metal fence | Blue, `#6EB8FF` | A 5 m metal strip with a pinned end; bends permanently without breaking. Access through MORE STARTERS. |
| Rock | Violet, `#ABA1F0` | Irregular stone with a buried central base. Uses the previous wall’s seam strengths; shells chip the perimeter while tanks cannot push the intact rock. |

The wall’s previous 16× seam-strength increase was rejected as too resistant. It now uses ordinary masonry strengths, the earlier brick size and one end support. In the centered trial, the first 6 kg / 220 m/s shell produces **14 chunks**, with the largest retaining **40%** of the original area. A 4500 kg tank approaching at 4.6 m/s produces **four larger chunks**, the largest retaining **47.5%**. Both trials measure one second after the input. Three centered shells produce 20 chunks; these counts characterize the standard setup, not every impact angle.

Wall values: density **2400 kg/m³**, depth **0.55 m**, friction **1.3**, seam tension/compression/shear **88 / 400 / 64 kPa**, grain cohesion **4**, fracture work **55 J/m²**, brick layout **0.7 × 0.6 m**, shell/contact resistance **1 / 1.5**. Edge 2 has a breakable support; more supports can be added in the editor. The original reference scene and legacy editor Barrier preset retain their earlier definitions. Reload the app and load the Wall starter to use the updated profile; saved drafts and copies retain their authored values.

The rock uses the previous wall’s seam tension/compression/shear **88 / 400 / 64 kPa**, modulus **15 GPa**, fracture work **55 J/m²**, cohesion **18**, and shell resistance **1**. Its density/depth remain **2900 kg/m³ / 0.65 m**, with **0.9 × 0.75 m** stone grains and contact resistance **1000**. In the centered shot trial it sheds about **0.53 m²** and retains an anchored **89%** core. Three shots aimed at the center and either side leave five chunks and an anchored **41%** core. Its central base stays fixed under head-on and ±34° tank approaches; detached stone chips can move. Anchoring changes the load path, so matching seam values does not imply the old wall’s exact fragment count.

These are effective game profiles, not calibrated engineering materials. `cargo run --release -p sim --example obstacles` compares the original barrier, former wall and updated objects under identical shots and a sustained tank push. Add `--oblique` to alternate shell targets across the object. `cargo run --release -p sim --example library` measures all current starters.

**Editing components**

**SHAPE → GROUND ANCHOR** toggles the buried central base. The cross marks the area centroid (or a nearby interior cell for a concave shape); the circle guides the central region, with radius one quarter of the smaller outline dimension. The central grain and grains whose centroids fall inside that region are attached. Editing the shape or grains rebuilds the base. Each attachment follows only its containing child during fracture refinement; detached chips do not gain new anchors. A tiny isolated anchored remnant still uses normal debris cleanup.

**PIN SELECTED EDGE** now toggles supports independently, so a wall can be held at both ends. Pins remain breakable material connections. The original `pinned_edge` field is preserved, with optional `additional_pinned_edges`; optional `ground_anchor` defaults to false for old files. Undo, draft/library storage and experiment export/replay retain both.

**SHAPE → COLOR** accepts six hexadecimal digits, optionally prefixed with `#`. `AUTO` uses the material palette. Color follows fragments, refined cells, saved recipes and comparison recordings. Stress view intentionally displays stress colors instead.

**MATERIAL → FRICTION / BOUNCINESS** controls contact response. Friction (0–2) supplies both surface friction and the approximate horizontal floor resistance, `deceleration = friction × 9.81 m/s²`. Bounciness (0–1) is collision restitution; larger values retain more relative separating speed. Rapier averages the contacting surfaces' coefficients. The fixed test backstop has restitution 0.08, so barrel values 0.04 and 0.70 give pair coefficients 0.06 and 0.39. Deformation and other losses can further reduce the actual rebound. Shells are absorbed/detonated, so this setting never turns them into bouncing balls.

Ground rotational friction is now **four times stronger** for movable objects and detached fragments. Spin deceleration is `10 × friction / 0.32` rad/s², clamped at zero; a barrel spinning at 4 rad/s takes about 1.42 s to stop without further impacts. The existing friction field still controls the material ratio. Each rigid chunk receives drag once, regardless of mesh detail. Fixed objects stay fixed, and the tank uses its separate track traction/braking model.

`ObjectAsset.restitution` is saved with the object, copied into every collider, and inherited through fracture/refinement and chunk rebuilds. Older version-1 files without it default to 0.04. The barrel's current contact values remain friction 0.09 and restitution 0.04. Its selected deformation profile is yield 500 Pa, flow 2/s, hardening 0, limit 3; this adoption applies to the ordinary barrel only. Existing saved objects retain their own values.

**MATERIAL → OBJECT BEHAVIOR** opens a separate page:

| Control | Effect / units |
| --- | --- |
| Shell resist × | Multiplies seam strength and crack-work cost, or plastic yield/work for deformable objects, under shell/blast loads; 0.1–1000. It does not multiply the physical impulse. |
| Contact resist × | Destructible seam strength/work multiplier for collisions; 0.1–1000. |
| Crushable | Enables buckling under opposing contact loads acting inward on separated faces. A one-sided shove does not enable it. |
| Crush force, kN | Minimum force on both opposed sides; 0.1–1000 kN. |
| Crush scale × | Multiplies contact resistance while squeezed; 0.0001–1. Destructible mode only; lower values allow easier fracture. |
| Explosive | Attaches a damage-triggered explosive to any authored shape. |
| Blast energy, kJ | Source work budget; 1–5000 kJ. Distance, occlusion and receiving geometry determine delivery. |
| Blast radius, m | Maximum pressure reach; 0.25–8 m. |
| Contact detail | Keep the initial grain/solver mesh, or allow collision-triggered local subdivision. Shell/blast subdivision remains available if stress/work thresholds are met. |

**PERMANENT DEFORMATION** opens the behavior selector and yield/flow/hardening/strain-limit settings. Barrels and the fence default to DEFORMABLE; walls and rocks to DESTRUCTIBLE. Deformable frames stay connected and do not refine under ordinary impacts; the [laser](AMMUNITION.md) can explicitly cut them. In this mode, contact resistance, crush scale and contact-detail controls are hidden; Crushable supplies the opposed-force gate. [PLASTICITY.md](PLASTICITY.md) describes the model and limits.

The usual outline, grain sites, brick size, support, density, thickness, friction, strength, E, Gc and cohesion controls remain available. Material palette buttons reset material constants but preserve these components and the explicit color. Invalid drafts cannot be tested, exported or saved over a valid copy. Undo/redo includes component and color edits. A comparison case can visit the editor for component changes and return through **COMPARE**; recipes preserve the full object definition.

**Shared yard and persistence**

Press **4 / 4 OBJECTS** in the proving ground, or **TEST OBJECT YARD** in the library. The yard contains both barrel types, masonry, a rock, a metal fence and a rigid backstop. It uses the built-in profiles; **TEST** uses your current edited draft. Custom scene placement is still deferred. Native code can construct small multi-object tests with `World::from_objects` (32 objects and 8192 initial piece records maximum, including supports/boundaries).

Custom objects use `objects/library.json` natively and browser local-storage key `fracture.objects.v1`. The library holds up to 24 copies and 1 MiB of JSON. It is separate from the editor draft and experiment library. Writes validate first; storage failures preserve the previous library. **FILE → EXPORT / IMPORT JSON** shares individual objects. Loading a starter/copy replaces the draft and can be undone. If existing library storage cannot be read, it is preserved; draft editing/export remain available.

Object JSON remains version 1: optional `color` and `components` fields default to the old behavior when absent. Components are additive; a rock can be explosive, and a custom polygon can be crushable. Runtime fuse identity is per spawned object and inherited by refined cells. Experiment engine revision is now `experiment-terrain-bridges-13`; old saved visual checkpoints remain loadable, while re-running uses current physics.

**Physical scope**

Failure still requires stress overload and available fracture work, without hit points. Contact impulses are not applied twice. The low crush scale is retained for destructible recipes. Deformable barrels instead acquire permanent dents through the yield/work model; neither mode implements shell FEM or rolling about an out-of-plane axis. The barrel's density is an effective hollow-vessel density applied to a solid polygon.

An explosive ignites on accepted permanent deformation or, in destructible mode, an actual internal seam failure, whether caused by a projectile, contact or another explosion. Breaking its support alone does not ignite it. Its vessel is consumed before pressure rays are cast so its own colliders cannot shield every neighbor. Each spawned vessel explodes once; neighbors ignite only if their own material deforms or fractures. There are no casing fragments or simulated fuel chemistry yet. Blast delivery uses the existing occluded rays, finite pressure pulse and bounded energy model. Ordinary dust cleanup is unchanged.

The next iteration is calibration: repeated/oblique shots, pinned versus free crushing, walls under different tank masses, rock core retention, and useful blast damage distances. The component controls and reproducible comparisons provide the starting point.

`node tools/library-smoke.mjs` exercises the picker, component/color editing, saved copies, reload, invalid-save preservation, shell/barrel explosions and yard navigation through actual browser input. It uses the same local Chrome/server setup as the other smoke scripts and restores the original local stores afterward.

**Older yield comparison:** [the ten-case direct link](http://127.0.0.1:8000/?experiment=barrels) opens ten reproducible tank/backstop trials. They vary plastic yield stress and hardening, with common geometry, mass and drive settings. Select a number, focus it, mark a favorite, then save or copy it into the editor. See [EXPERIMENTS.md](EXPERIMENTS.md) for the matrix and direct launch link.

**DEFORMATION / 6** opens six barrels with limits 0.4 / 0.8 / 1.2 / 1.6 / 2.2 / 3.0 under identical test settings. Yield 0.5 kPa, flow 2/s, hardening 0 and all other object properties match. The shared toolbar changes the test's speed, angle, offset, mass, duration and backstop at runtime. Use **FOCUS CASE → MATERIAL** to tune one barrel or **USE CASE IN OBJECT EDITOR** to save a library copy. The ordinary barrel starter now uses the selected case 6: yield 0.5 kPa, flow 2/s, hardening 0, limit 3. Explosive-barrel and generic plasticity defaults remain separate.

**SHOT RESPONSE / 6** opens the friction/bounciness matrix with a shared shell impact. See [EXPERIMENTS.md](EXPERIMENTS.md) for the six values and runtime controls.

The selected contact references are case 6 with the close backstop and case 2 without it. These remain separate experiments; the ordinary barrel retains case 2 base values and adds the soft-impact curve described below. No fixture-dependent material switching is used.


**Soft-impact bounce:** the ordinary barrel uses the selected wall-release case 2: `soft_bounce: {restitution: 0.62, speed: 16}` to base restitution 0.04 and friction 0.09. In **MATERIAL → BOUNCE >**, enable/disable the curve and edit both values. Gentle contacts use the soft coefficient, contacts faster than twice the soft speed use base bounce, with a smooth transition in between. Relative normal contact speed includes rotation. The two material coefficients are averaged before collision solving. No extra separating impulse is applied, and ground friction is independent of this curve. The intent is visible wall separation with dissipative hard collisions. This is a tunable 2D game approximation; it does not model stored elastic strain energy or calibrated metal restitution. Old files without the optional property retain constant bounce. The property is inherited by refined cells and preserved during chunk rebuilds.
