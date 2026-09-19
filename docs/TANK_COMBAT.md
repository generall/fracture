# Tank combat: first playable model

Level play has one player tank and one AI opponent, each using the existing 4.5-tonne rigid hull, track forces, terrain traction, recoil and swept projectiles. Activity is selected in the level editor; maps with no opposing spawn remain solo tests. Other laboratory scenes retain their existing tank behavior.

## Implemented damage

Each tank starts with 100 hull health. A shell's incoming direction relative to the hull chooses armor resistance: front 100 kJ, side 55 kJ, rear 30 kJ. Front and rear each span 120 degrees. These are effective gameplay values, not claims about real armor thickness.

Direct damage is `max(0, kinetic_energy * penetration_factor - armor_resistance) / 6000`, capped at 85 health per direct hit. AP uses factor 1, HE 0.65, MG 0.5. Standard AP direct hits remove about 65 / 72.5 / 76.7 health at the front / side / rear before secondary blast damage. Standard MG kinetic energy stays below all armor thresholds. Incoming mass and speed affect the result.

Blasts use the existing occluded pressure rays and distance attenuation. Intercepted energy is aggregated per tank, then divided by front / side / rear resistance of 3000 / 2200 / 1500 J per health point. AP's secondary blast contributes at 15% of this damage; HE and exploding scenery use 100%. A laser applies 4 health per cutting pulse (six pulses per second); it models sustained heating rather than ballistic penetration. Ramming also damages hull health in proportion to collision energy. Normal impact work is estimated from the actual collision impulse and closing velocity, aggregated across hull and track contacts, and capped by kinetic energy lost in the collision. Impacts delivering less than 1 kJ per tank are ignored. Above that threshold, damage is linear: 1 health per 3 kJ at the front, 2.2 kJ at the side, or 1.6 kJ at the rear. For comparable collisions, doubling mass roughly doubles damage and doubling speed roughly quadruples it. Front impacts cost less health than side or rear impacts. Tank-to-tank collisions can damage both tanks; collisions with walls or heavy debris can damage the tank. Gentle contact, sliding and steady pushing do not continuously drain health. Scenery still breaks through its existing physical stress model.

At zero health the tank stops driving, firing and capturing, and its colliders are disabled. A visual explosion marks destruction; it does not trigger an extra damage chain. After three seconds, the same tank body returns with full health, zero velocity and one second of damage protection. The original spawn is preferred; nearby positions are checked against the actual hull and tracks. A completely blocked spawn area waits. No accumulating wrecks obstruct the map.

## AI limits

A deterministic A* route uses tank clearance and terrain cost, blocks fixed obstacles and chunks of at least 4.5 tonnes, and assigns extra cost to lighter breakables. The actual track controller follows the route. It periodically replans and reverses briefly if stuck. The grid is conservative: very narrow passages can be considered impassable, and a chosen light obstacle is an attempted ram, not guaranteed to break.

Attack samples player position every 0.35 seconds, requires 0.7 seconds of uninterrupted visibility, turns the turret at about 60 degrees per second, and fires every 1.6 seconds at most. Aim spread is deterministic, up to 2 degrees stationary or 4 degrees moving. Conquer uses the same shooting limits within 12 metres while heading toward the player flag. Static and Driving never fire. The player retains the existing weapon controls and reload cadence.

## Suggested next damage extensions

- Track damage from hits to the track colliders: reduce one track's traction and turning ability, with a repair timer.
- Engine damage from rear penetrations: reduce available drive force rather than imposing an arbitrary speed cap.
- Turret damage: slow traverse or increase reload time after a penetrating turret hit.

These would build on the current directional hit classification. Hull health provides a clear destruction condition while component damage adds consequences before destruction. Component damage, repair controls and persistent wrecks are not implemented yet.

Code: `sim/world/combat.rs`, `world/enemy.rs`, `world/enemy/navigation.rs`, plus weapon, vehicle and level integration. Focused checks: `cargo test --locked -p sim world::combat` and `cargo test --locked -p sim world::level::tests`.
