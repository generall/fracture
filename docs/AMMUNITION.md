# Weapons and pickups

The main gun uses **HE** or **AP**. Select with **5 / 6**, the HUD buttons, or **B** to swap. Left-click or **F** fires once per press. Changing round type starts a full **0.9-second reload**, even if the gun was ready. Selecting the already loaded type does not restart it. Machine gun has been removed.

| Weapon | Availability | Profile |
| --- | --- | --- |
| HE | Main gun | 6 kg at 220 m/s; 300 kJ blast, 1.7 m radius; 0.9 s reload. |
| AP | Main gun | 8 kg at 350 m/s; 800 kJ concentrated blast, 0.65 m radius; 0.9 s reload. |
| Rocket | Configurable-charge pickup; G / right-click | 16 kg at 12 m/s; 5.5 m explosion radius, 20 MJ external blast plus a bounded 4 MJ internal shock in the directly struck connected material. Can shatter the default rock into rubble. |
| Laser | Configurable-charge pickup; G / right-click | An uninterrupted 3-second, 0.6 m wide beam reaching 40 m. Follows actual turret direction with zero dispersion. Ten cutting pulses per second, up to eight small-cell operations per pulse, and 12 tank damage per pulse. |
| Plasma cannon | Configurable-charge pickup; G / right-click | Charges for 1 second, then releases one 0.6 m wide, 40 m pulse with the full laser budget: 30 cutting pulses (up to 240 small-cell operations, 360 total tank damage). Zero dispersion and no mechanical impulse. |
| Mine | Configurable-charge pickup; G / right-click (LT on controller) | Drops beneath the vehicle, arms after 0.75 s and disables an enemy vehicle on footprint contact. Friendly vehicles pass safely. |

Mines give their team a **1 m radius of sight**, blocked by solid cover. Friendly mines remain visible to their team. An enemy mine appears only while a living allied vehicle is within **3 m** with a clear sight line; flags, radar and remembered fog do not expose it. Leaving that range hides it immediately. HE/AP shells, rockets, scout bullets, laser/plasma beams and direct mortar impacts can destroy mines; solid cover intercepts ground-level shots. Shooting disarms a mine without triggering vehicle damage. Placed mines survive their owner's death. Both bundled battlefields have mine pickups near each team's spawn, supplying three charges with a one-second placement cooldown and a 60-second pad respawn. Mines are also included in supply drops.

The player starts with an empty special slot. Drive over a pickup to collect it. A full slot leaves further pickups on the ground. G or right-click consumes one charge, independently of the main-gun reload. The carried batch retains its pickup’s charge count and use cooldown; the slot becomes empty after the last charge. A blocked rocket muzzle retains its charge and does not start cooldown. The laser burst and plasma charge cannot be released or cancelled; movement and turret aiming remain available, while main-gun fire is blocked. Plasma fires along the actual turret direction at discharge. Death cancels either weapon and discards inventory. Pause and slow motion affect all timers.

In the level editor, choose **Special → Laser / Rocket / Plasma / Mine** to place a non-solid pickup pad. Select it to set **Charges per Pickup** (1–99), **Use Cooldown / Seconds** (0–600) and **Respawn / Seconds** (0.1–600). New pads default to 1 charge, a 5-second use cooldown and a 20-second respawn. Rocket cooldown starts at firing; laser cooldown starts after its full three-second burst; plasma cooldown starts at discharge, after its one-second charge. The HUD displays charges and remaining cooldown. Older files without these settings retain single-use weapons with no additional cooldown. Pads start available and begin their countdown on collection. Fog of war remembers their last observed availability. Pads do not block movement, sight or shots. The level camera follows the tank with mouse look-ahead. Material test scenes use **middle-drag** to pan.

Rockets use the existing fracture/material system. The direct hit transmits a short-range internal shock through the struck connected material, limited by seam strength, fracture work and an energy budget. The external explosion respects solid cover; unbreakable walls remain intact. Rockets expire after eight seconds if they hit nothing.

The laser uses a swept 0.3 m radius for collision. Unbreakable cover stops it. Each cutting operation refines a coarse cell or removes a leaf no larger than 0.0125 m²; existing refinement limits still apply. Cutting adds no mechanical impulse. Layered glow, moving filaments, a bright core and impact sparks show the beam. Aim spread from motion and firing does not affect it.

Plasma shares the laser’s cover and refinement rules, resolving the full cutting budget at one simulation tick. A violet muzzle orb grows during charging, then a brief violet pulse fades without dealing additional damage. Moving targets and falling debris can make the result differ from a laser spread over three seconds.

Material experiments can select all five weapon profiles directly without pickups; normal player controls expose only HE/AP and the collected special.
