# Tank combat: first playable model

Level play has one player tank and one AI opponent, each using the existing 4.5-tonne rigid hull, track forces, terrain traction, recoil and swept projectiles. Activity is selected in the level editor; maps with no opposing spawn remain solo tests. Other laboratory scenes retain their existing tank behavior.

## Implemented damage

Each tank starts with 100 hull health. A shell's incoming direction relative to the hull chooses armor resistance: front 100 kJ, side 55 kJ, rear 30 kJ. Front and rear each span 120 degrees. These are effective gameplay values, not claims about real armor thickness.

Direct damage is `max(0, kinetic_energy * penetration_factor - armor_resistance) / 6000`, capped at 85 health per direct hit. AP uses factor 1, HE 0.65, MG 0.5. Standard AP direct hits remove about 65 / 72.5 / 76.7 health at the front / side / rear before secondary blast damage. Standard MG kinetic energy stays below all armor thresholds. Incoming mass and speed affect the result.

Blasts use the existing occluded pressure rays and distance attenuation. Intercepted energy is aggregated per tank, then divided by front / side / rear resistance of 3000 / 2200 / 1500 J per health point. AP's secondary blast contributes at 15% of this damage; HE and exploding scenery use 100%. A laser applies 4 health per cutting pulse (six pulses per second); it models sustained heating rather than ballistic penetration. Ramming also damages hull health in proportion to collision energy. Normal impact work is estimated from the actual collision impulse and closing velocity, aggregated across hull and track contacts, and capped by kinetic energy lost in the collision. Impacts delivering less than 1 kJ per tank are ignored. Above that threshold, damage is linear: 1 health per 3 kJ at the front, 2.2 kJ at the side, or 1.6 kJ at the rear. For comparable collisions, doubling mass roughly doubles damage and doubling speed roughly quadruples it. Front impacts cost less health than side or rear impacts. Tank-to-tank collisions can damage both tanks; collisions with walls or heavy debris can damage the tank. Gentle contact, sliding and steady pushing do not continuously drain health. Scenery still breaks through its existing physical stress model.

At zero health the tank stops driving, firing and capturing, and its colliders are disabled. Disconnected physical hull and track pieces replace the tank; destruction does not trigger an extra damage chain. After ten seconds, the same tank body returns with full health, zero velocity and one second of damage protection. The original spawn is preferred; nearby positions are checked against the actual hull and tracks. A completely blocked spawn area waits. Wrecks clear before the tank respawns.

## AI limits

Player vision and AI sight ignore physical props and disconnected chunks with a footprint below **0.5 m²**. They are excluded before occluder bounds, silhouette extraction and ray casting. Size uses the whole connected chunk, so small mesh triangles in an intact wall still provide cover; authored indestructible objects are measured as a whole despite their separate triangle bodies. Detached fragments are reclassified immediately. This sight rule leaves collision, projectile impacts and grass concealment unchanged.

A deterministic A* route uses tank clearance and terrain cost, blocks fixed obstacles and chunks of at least 4.5 tonnes, and assigns extra cost to lighter breakables. The actual track controller follows the route. It periodically replans and reverses briefly if stuck. The grid is conservative: very narrow passages can be considered impassable, and a chosen light obstacle is an attempted ram, not guaranteed to break.

Attack samples player position every 0.35 seconds, requires 0.7 seconds of uninterrupted visibility, turns the turret at about 40 degrees per second, and fires every 1.6 seconds at most. Enemies use the same movement-dependent dispersion as the player. Conquer uses the same shooting limits within 12 metres while heading toward the player flag. Static and Driving never fire. The player retains the existing weapon controls. HE/AP reload takes 0.9 seconds; switching HE/AP starts a fresh reload. Laser and rocket are single-use pickups activated with G or right-click.

Attack and Conquer also shoot breakable cover blocking their route. They prefer a route around heavy cover, but can plan a breach when no such route reaches the goal. A sweep checks the tank's width, including the edges of partial openings. The tank stops to aim, uses HE beyond 3.3 metres (AP closer), and tries to reverse if the obstruction is too close. Acquisition, turret speed and firing cadence still apply. It replans after shots and resumes driving as the opening clears. A fully acquired player takes priority; brief glimpses through rubble do not interrupt breach aiming. Fixed walls are never demolition targets.

## Suggested next damage extensions

- Track damage from hits to the track colliders: reduce one track's traction and turning ability, with a repair timer.
- Engine damage from rear penetrations: reduce available drive force rather than imposing an arbitrary speed cap.
- Turret damage: slow traverse or increase reload time after a penetrating turret hit.

These would build on the current directional hit classification. Hull health provides a clear destruction condition while component damage adds consequences before destruction. Component damage and persistent wrecks are not implemented yet.

Code: `sim/world/combat.rs`, `world/enemy.rs`, `world/enemy/navigation.rs`, plus weapon, vehicle and level integration. Focused checks: `cargo test --locked -p sim world::combat` and `cargo test --locked -p sim world::level::tests`.

## Physical wrecks

At destruction, the tank becomes ten disconnected rigid hull and track pieces. Their combined mass matches the tank, and they inherit its linear and angular motion. A lethal shell or blast transfers its incoming impulse to the pieces near the impact; ramming uses the motion already resolved by the collision solver. Hull and track fragments keep their beveled silhouettes, engine vents, and frozen tread slats as they move and rotate. There are no scripted paths for these physical pieces or overlapping turret/hull colliders.

New wrecks always spawn. Playable levels use a soft target of 8,192 active pieces: every quarter-second, cleanup retires the smallest disposable chunks far outside all tanks' sight ranges, both flag lookouts and owned radar coverage, with an extra 8 m margin. Equally small chunks are removed farthest-first. Whole chunks are removed together; intact, anchored and connected structures stay. If only protected pieces remain, the target can be exceeded. Expired records do not count toward this live budget, and each tank's wreck still expires before it can respawn.

Wreck pieces collide with tanks, scenery, other debris and weapons. They use ordinary floor friction, rendering and fog visibility, including inheriting visibility from an observed destroyed tank. Each piece uses the normal debris lifetime: 2.5 seconds total, fading during the final second, then its physics body and collider are removed. The original tank still respawns after ten seconds.

Observed destruction also emits animated fire and drifting smoke, plus a cosmetic turret and barrel that arc away from the impact with a shadow, spin and landing bounce. Explosive hits give the largest toss; ramming gives a smaller lift and fire. The turret uses the original tank silhouette, hatch and current aim. These effects add no physics bodies or damage, obey pause/slow motion, and fade with the 2.5-second wreck lifetime. Hidden enemy deaths do not reveal effects.

## Aiming

The player turret turns toward the cursor at 60°/s; enemy turrets turn at 40°/s. The cursor crosshair marks the requested aim, while a borderless, 7-pixel-wide ring sector shows the possible shot directions at a fixed 2.5 m radius from the muzzle, independent of cursor distance. Its translucent cyan fill (45% opacity) advances across the muted ring during reload and fills the entire sector when ready; it replaces the bottom-bar reload animation. The barrel, muzzle obstruction checks, shots and player vision all use the actual turret angle.

Settled accuracy is ±2°. Driving at normal top speed adds up to 24°, turret traversal adds up to 8°, and hull rotation adds up to 10°, with a total half-angle capped at 40°. Each fired HE/AP shell adds 6° of spread, a rocket adds 8°, and the laser adds no spread, subject to the same cap. Unsuccessful fire attempts add no spread. Spread grows quickly (0.03 s response) and recovers gradually (0.85 s response) after stopping, approaching settled accuracy over a few seconds. Shells sample deterministic per-tank dispersion inside this cone; the special laser follows the exact turret angle regardless of spread. Turret and indicator rotation interpolate between simulation ticks. Scripted material trials without aim commands retain their precisely specified shot directions.
