**Ammunition and cutting**

Select with **5–8**, click the four ammunition slots, or press **B** to cycle. Aim with the mouse and fire with left click or **F**. Hold for continuous machine-gun fire or laser cutting. HE and AP fire once per press. Reload is shared across weapons; switching cannot bypass it. While paused, a press queues one shot/pulse for the next step or resume.

| Key | Weapon | Default projectile | Kinetic energy | Impact blast | Firing |
| --- | --- | --- | --- | --- | --- |
| 5 | High explosive / HE | 6 kg, 220 m/s | 145.2 kJ | 300 kJ, 1.7 m radius | 0.45 s reload |
| 6 | Armor piercing / AP | 8 kg, 350 m/s | 490 kJ | 800 kJ, 0.65 m radius | 0.45 s reload |
| 7 | Machine gun / MG | 24 g, 850 m/s | 8.67 kJ | None | 12 rounds/s while held |
| 8 | Laser | Beam, no projectile mass | No mechanical impulse | None | 6 cutting pulses/s while held |

HE retains the accepted impact, blast and recoil behavior. AP supplies more kinetic work and a much more concentrated blast at the hit point. Its blast radius remains 0.65 m, but its source energy rises from 40 to 800 kJ so the small footprint delivers stronger local damage. In the standard wall test it creates 16 fractures within 0.65 m of impact versus HE’s 12; on the rock the counts are 3 versus 1, and on the ordinary plate 16 versus 4. Both are consumed at the first impact; this iteration does not model residual projectile penetration. MG uses a 4 mm swept radius versus the shells' 35 mm radius. All projectiles disappear on impact or expire after three seconds in flight. Colored tracers distinguish HE, AP and MG. Their launch momentum has equal/opposite tank recoil, and the existing energy budget limits damage and recipient motion.

Machine-gun resistance comes from thickness, seam strengths, fracture work and the object's component multipliers, without a heavy/light tag or accumulated health. The new MG round has six times the old kinetic energy. In the two-second standard test, 24 bullets dent the ordinary barrel, bend the metal fence, ignite an explosive barrel and break a 2 cm panel. The wall acquires a few cracked seams but remains one chunk; the rock stays uncracked. The bullets themselves never explode; an explosive barrel’s own fuse supplies its blast. Changing the target's material or bullet parameters can change that result.

**Laser operation**

The beam stops at the first live surface, including uncuttable arena walls and support fixtures. It acts on destructible material and deformable frames. Cutting runs at 6 operations per second instead of 30, a fivefold slowdown. The beam still follows aim at fixed simulation ticks while held, so lowering the cutting rate does not make aiming stutter. Each cutting pulse performs exactly one operation:

1. If the hit triangle exceeds **0.0125 m²**, subdivide it into four connected children. The new local grid becomes visible; subdivision alone neither detaches nor moves material.
2. Otherwise remove that one triangle, its collider and incident connections immediately. Rebuild the surviving connected chunks and their mass/inertia.

Holding the beam at the same point advances through the newly exposed surface. This is adaptive triangular detail, not a world-aligned square grid. The area threshold is about an 11 cm square equivalent; triangle shapes and lengths follow the authored mesh. Existing finer cells can be removed immediately. Refinement is bounded by **eight generations** and **8192 total cell records**, including retired parents. At the limit, an oversized cell remains and the HUD reports the detail limit; the laser never erases a whole coarse cell as a fallback.

Cutting applies no recoil, impact impulse, blast or fracture-work load. Survivors inherit their previous rigid velocity field; removed material takes its own mass and momentum out of the simulation. Ground attachments follow only their containing child. A completed cut can detach anchored material or sever a deformable frame. Laser removal itself does not ignite explosive fuses. Refining already-fading debris preserves its cleanup deadline. Detached small debris still follows normal timed cleanup; the **CUT** counter and removed area are separate from that cleanup.

The accepted wall, rock and barrel recipes are unchanged. Cutting changes the running test only; reset restores its initial asset.

**Reproducible tests**

In the six-view **TEST ALL CASES** bar, choose **SHELL**, then cycle **AMMUNITION**. In a focused comparison, **IMPACT → AMMO … >** cycles the selected case; **COPY IMPACT TO ALL CASES** shares it. HE/AP trials fire once at the start; MG/laser trials hold fire for the chosen duration. Mass and speed fields use the selected projectile's ranges and are hidden for laser. Shared controls preserve the different object properties.

Recipes store `impact.ammunition`; missing values in older JSON mean `high_explosive`. Checkpoints retain laser beam endpoints, the cut geometry and `metrics.ablated_area`. Same-build replays reproduce the recorded frames. The engine revision is `experiment-terrain-bridges-13`. Existing checkpoints remain readable; RUN recomputes with the current build. Previously saved MG recipes retain their authored mass/speed: reselect MG to use the stronger default round.

`Shot::new(ammunition, target)` supplies default mass/speed. Explicit `Shot` values can customize them within the ammunition's supported ranges. `cargo run --release -p sim --example ammunition` compares the accepted starters and a thin panel. `node tools/ammunition-smoke.mjs` exercises actual browser selection, held fire, pause/step, cutting and saved comparisons.
