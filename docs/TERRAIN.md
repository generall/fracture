# Terrain and bridges

Press **9** or choose **TERRAIN** to open the river crossing. **R** restores the bridge and tank. The other scenes and saved object comparisons still use ordinary ground.

The road leads across the river. Sand occupies the northwest and southeast corners; the remaining banks are default ground. The driving HUD names the surface under the tank. All terrain is drawn with colored shapes and lines, without textures. Adjacent and overlapping tiles share one exposed outline; sand, water and bridge titles are hidden. The A/B booth markers remain.

| Surface | Tank behavior |
| --- | --- |
| Ground | Top speed 6.5 m/s, with the existing traction and braking forces. |
| Road | 1.45× speed limit (9.425 m/s), 1.3× engine force, 1.4× grip/braking capacity and 1.35× lateral damping. |
| Sand | 0.48× speed limit (3.12 m/s), 0.6× engine force, 0.65× grip/braking capacity and 0.8× lateral damping. |
| Water | Impassable to the tank. Collision checks use the complete hull and tracks, including diagonal approaches. |
| Intact bridge | Road response; permits crossing over water. |

The four track contact patches sample terrain independently. Each track's front/rear samples determine its target speed; each patch supplies its own traction and braking force. Speed changes come from forces, not instantaneous velocity resets. Default ground retains the original force calculations. Object material friction and the accepted barrel, wall, rock and ammunition profiles are unchanged.

## Breaking a bridge

**A** and **B** mark the support booths beside the entrances, one on each bank. Only these booths are weapon targets: HE, AP, MG, laser and blast rays pass over the deck from either side. A shot can hit an object beyond the bridge. The laser can cut booth material using the same subdivision and removal rules as other objects.

The booths now resist ordinary pushing and low-speed impacts. The bridge recipe's contact resistance is **1000** (previously inherited wall value **1.5**); projectile resistance remains **1**. Ten-second trials check pushing, 2–3 m/s direct bumps and 4.6 m/s glancing impacts. A forceful direct ram can still sever the support. This changes the contact fracture threshold/work, not the tank's impulse or the bridge's connectivity rule.

The deck and both supports form one destructible masonry mesh, pinned at two bank fixtures. Each connected material component uses the existing rigid-chunk and elastic stress solver. A small chip does not automatically collapse the bridge. The span fails when **no surviving material component remains connected to both bank fixtures**. This also detects a complete cut across the deck even if each half is still anchored separately. There are no bridge health points or hit counters.

At failure, the crossing closes immediately. The fractured span falls and fades into the water over 1.4 simulated seconds; the submerged material leaves collision and fracture processing. The bank fixtures remain. If any part of the tank's physical footprint is over the newly exposed water, the tank falls with the bridge. Movement and firing are disabled; **R** rebuilds the scene. A tank entirely on a bank survives.

This first implementation models the vertical layer explicitly as a gameplay rule in the horizontal simulation: tanks drive above the deck, exposed support material can be rammed, and all weapons travel above the deck. Blasts load the exposed booths, whose structural stress can still propagate through the connected span. Water collision barriers affect the tank and do not intercept shells, laser beams or blast rays.

## Implementation scope

The range recipe is authored in `crates/sim/src/world/scenes/terrain.rs`; crossing and collapse live in `world/terrain.rs`, and collision/weapon targeting rules live in `world/collision.rs`. Public `TerrainKind`, `TerrainPatch` and `BridgeView` provide rendering and inspection data; `World::terrain_at`, `bridges`, `tank_alive` and `tank_fall_progress` expose current state. Patch bounds, road/sand traction multipliers and the bridge material recipe live in Rust for this first version. The object editor and comparison-file format do not yet author terrain maps.

This is a single-span 2.5D support model, not a vertical rigid-body/fluid simulation. It does not yet calculate deck sag or weight capacity, independent falling sections, or local holes that swallow a tank before the complete span loses support. Loose objects retain their ordinary horizontal-floor behavior. Water blocks the tank from entering normally; support loss beneath an occupant triggers sinking.

Eleven regression tests cover unchanged ground, road/sand movement and road grip, water at speed and angles, crossing in both directions, gentle-contact survival and forceful shell/ram destruction, untargetable deck and blast layers, local cutting versus complete severing, collision-layer inheritance through refinement, sinking and disabled controls, closing the crossing, ballistic transparency and deterministic command replay. `node tools/terrain-smoke.mjs` exercises the actual browser scene, crossing, all four weapons passing over the deck, booth shooting from the bridge, sinking and rebuild.
