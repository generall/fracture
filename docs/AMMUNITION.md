# Weapons and pickups

The main gun uses **HE** or **AP**. Select with **5 / 6**, the HUD buttons, or **B** to swap. Left-click or **F** fires once per press. Changing round type starts a full **0.9-second reload**, even if the gun was ready. Selecting the already loaded type does not restart it. Machine gun has been removed.

| Weapon | Availability | Profile |
| --- | --- | --- |
| HE | Main gun | 6 kg at 220 m/s; 300 kJ blast, 1.7 m radius; 0.9 s reload. |
| AP | Main gun | 8 kg at 350 m/s; 800 kJ concentrated blast, 0.65 m radius; 0.9 s reload. |
| Rocket | Single-use pickup; right-click | 16 kg at 12 m/s; 5.5 m explosion radius, 20 MJ external blast plus a bounded 4 MJ internal shock in the directly struck connected material. Can shatter the default rock into rubble. |
| Laser | Single-use pickup; right-click | An uninterrupted 3-second, 0.6 m wide beam reaching 40 m. Follows actual turret direction with zero dispersion. Ten cutting pulses per second, up to eight small-cell operations per pulse, and 12 tank damage per pulse. |

The player starts with an empty special slot. Drive over a pickup to collect it. A full slot leaves further pickups on the ground. Right-click consumes the carried weapon, independently of the main-gun reload. A blocked rocket muzzle retains the pickup. The laser cannot be released or cancelled; movement and turret aiming remain available, while main-gun fire is blocked during its burst. Death ends the beam and discards inventory. Pause and slow motion affect all timers.

In the level editor, choose **Special → Laser / Rocket** to place a non-solid pickup pad. Select it to set **Respawn / Seconds** (0.1–600; default 20). Pads start available and begin their countdown on collection. Fog of war remembers their last observed availability. Pads do not block movement, sight or shots. The level camera follows the tank with mouse look-ahead. Material test scenes use **middle-drag** to pan.

Rockets use the existing fracture/material system. The direct hit transmits a short-range internal shock through the struck connected material, limited by seam strength, fracture work and an energy budget. The external explosion respects solid cover; unbreakable walls remain intact. Rockets expire after eight seconds if they hit nothing.

The laser uses a swept 0.3 m radius for collision. Unbreakable cover stops it. Each cutting operation refines a coarse cell or removes a leaf no larger than 0.0125 m²; existing refinement limits still apply. Cutting adds no mechanical impulse. Layered glow, moving filaments, a bright core and impact sparks show the beam. Aim spread from motion and firing does not affect it.

Material experiments can select all four weapon profiles directly without pickups; normal player controls expose only HE/AP and the collected special.
