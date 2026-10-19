**Sandbox implementation and verification**

Rust, Rapier 2D 0.35.3, and Macroquad 0.4.16; transitive dependencies are pinned by Cargo.lock. The simulation has no rendering or runtime network dependency. Presentation uses a nearly black field, cyan/violet geometry, bright shell effects, and bold high-contrast text.

**Geometry and fracture.** The barrier starts with 252 triangular cells and 367 seams, including attachments to two fixed end plates, but its material is one fixed compound rigid body. The cantilever starts with 148 cells in one supported chunk. The rock field starts with 15 dynamic chunks of seven cells each. The tank is another dynamic body. Cells are internal material geometry; chunks are the independently moving connected components.

A deterministic jittered lattice supplies the barrier and cantilever; irregular radial fans supply the rocks. Exact triangle tiling replaces the former 1.5% seam gaps. Mass is area × depth × density, with default depth 0.25 m and rock density 2,600 kg/m³. Render and collision vertices agree. Rock grain interiors have 30 times the strength and fracture-work requirement of seams between grains. Authored objects use their own density, depth, friction, strengths and cohesion.

The custom elastic graph estimates seam forces from shell, pressure and Rapier contact loads. It accounts for support constraints, rigid acceleration and bending moments. Stress overload plus available fracture work permits a crack; stress is recalculated before the next crack. Graph components become rigid bodies with conserved mass and inherited rigid motion. Strong impacts can locally refine a large triangle into bonded children, including cells inside an intact object. There are no physical joints between cells. See [the destruction architecture](DESTRUCTION.md) for equations, source budgets, refinement guards and limitations.

**Tracks and floor.** The plane is horizontal with zero in-plane gravity. Rubble has approximate Coulomb resistance against the unmodeled floor, clamped to avoid reversing velocity or adding energy. The 4,500 kg tank has a compound collider: a central hull and two tracks. Each track has two ground contacts, at its front and rear. Longitudinal traction and lateral scrub act at these points, so lateral friction produces a resisting torque during rotation. Engine force is limited to 7.2 kN total (about 1.6 m/s² initial acceleration); braking has a separate, larger force budget. Combined contact force is traction limited. Straight driving targets 6.5 m/s (saved comparison trials retain their authored speed). Steering scales both requested track speeds to keep their difference below 1.64 m/s; a zero track command remains zero. Zero throttle engages the brakes. The release test requires less than 0.08 rad (4.6°) of extra rotation and angular speed below 0.01 rad/s after 0.25 s. WASD mixes the track throttles, while Q/Z and E/C override them individually. There is no velocity teleportation. The bevels, turret, barrel recoil animation, and tread slats are visual details; the turret/barrel are not articulated rigid bodies. The barrel path is checked for obstruction before firing.

**Shells and explosions.** A shell is a ballistic state swept as a 3.5 cm radius shape against live colliders each microstep. The sweep includes target linear velocity and selects the earliest hit. Bounding boxes prune candidates before polygon queries. Angular motion is updated between microsteps; this is not a general continuous collision guarantee for rotating targets. Default ammunition is 6 kg at 220 m/s, with a 0.45 s reload. Shell mass/speed ranges are 1–20 kg and 80–400 m/s. MG instead uses 2–30 g and 150–900 m/s, with a 4 mm radius. AP has a smaller blast; MG has none. See [AMMUNITION.md](AMMUNITION.md) for profiles and the force-free laser path. Shells receive no floor friction, are consumed on impact, and expire after three seconds if they miss the field.

Firing applies equal and opposite shell/recoil momentum in isolation. A hit supplies its momentum and kinetic-energy budget to the struck cell. HE detonation separately supplies 50 kJ per kg of shell mass (300 kJ by default), over a 1.7 m default radius scaled by the cube root of mass. Each of 192 angular rays carries a fixed share, reduced quadratically with distance, to its first surface. Solid anchors absorb the share; destructible supported material can receive a structural load. Missed energy is lost and never redistributed to other targets.

Received pressure acts for 35 ms. Each ray contributes nominal impulse equal to its received energy divided by 80 m/s. These vectors are summed per cell with their original resultant line of action, preserving total impulse and torque across different surface tessellations. This is a tuned pressure coupling ratio. Crack work and positive kinetic work share each packet's finite budget. Point-impulse work includes angular motion, so a tiny fragment cannot turn shell momentum into unlimited energy. Source loads and ongoing pulses retain their work budgets and material contact points through refinement and splitting. Rapier contact loads are inspected after collision resolution and are not applied again.

This remains sampled 2D pressure, without wave travel, diffraction, reflections, penetration, gases entering cracks or physical shell shrapnel. Rays are sampled at detonation, not resampled as targets move during the pulse. Virtual elastic energy is not released into fragment motion. The mechanical coupling and material values are sandbox tuning. Momentum in consumed shells and unmodeled gases is not tracked. Sparks, tracers and impact rings are cosmetic.

**Debris lifetime.** After topology changes, unanchored components with combined area strictly below 0.075 m² receive a 3.0–3.9 simulation-second lifetime. They fade during their final second, then bodies and colliders are removed. Small cells bonded to larger structures stay. Larger detached triangles can acquire new internal seams on later strong impacts. Fixed anchors and the tank never expire. Cleanup deliberately removes mass and any residual motion. Stable cell records, including retired parents, remain until reset; the runtime refinement budget is 8,192 total records and eight generations per lineage.

**Integration and presentation.** Physics advances at 120 Hz with four explicit substeps. Rapier runs one TGS step and 12 PGS sweeps per substep. Dynamic chunks and the tank use CCD. Shell and pulse loads are resolved before each Rapier step; contact loads follow it. Impulse-to-force conversion uses the actual microstep duration. Static equilibrium is validated, but impact fracture is not timestep converged.

Wheel input is normalized by browser delta mode, then zoom eases using wall time while preserving the cursor's world point. The renderer interpolates previous/current poses; pause displays the current pose. Slow motion changes accumulated time without changing physics steps. Long stalls cap catch-up and display a notice. H shows dynamic bodies, material chunks, active cells and subdivision counts, plus a message if a refinement or stress-solver limit was reached. A failed stress solve is not used for fracture.

Scene layouts and insertion order are fixed for reproducible experiments on the same build. Cross-platform bit-identical replay is not claimed. The brittle model has no visible elastic deformation or full cohesive crack opening; barrels/fences use a separate permanent plastic-frame model. Bridges use a 2.5D collapse rule. Fatigue, free-form crack paths and general vertical physics remain deferred. Mesh detail cannot soften an intact rigid chunk, but exact crack patterns are still mesh dependent.

**Checks.** The current contribution refactor passes 98 simulation tests and two camera tests; see [HANDOFF.md](HANDOFF.md) for the latest verification. Simulation checks cover rest stability, gentle loads, seam section scaling, known axial/shear/bending loads, load transfer to a remote support, cached stresses under rotation, rigidity and mass across editor detail levels 0–4, and conservation of mass/linear momentum/angular momentum/kinetic energy during refinement and splitting. They also cover support-face preservation, no healing of existing cracks, no fracture without available work, repeated subdivision of an isolated triangle into clearable debris, local wall breaches, rock chunk retention, contact and projectile behavior, energy budgets, shielding, mesh-independent blast resultants, recoil, independent tracks, braking, deterministic replay, and editor geometry/JSON validation. Two camera tests cover smooth zoom and frame-rate independence.

The central native rock reference hit must leave at least two substantial target chunks and eleven of fifteen rocks intact after eight simulation seconds. Four oblique trajectories must preserve at least ten intact rocks. All five scenarios permit at most 0.225 m² of cleared rock. Area replaces raw deleted-cell count because subdivision changes the cell count. These are regression scenarios, not guarantees for every material and impact.

**Historical native cost, 2026-09-13 (earlier reference meshes).** Linux x86-64, AMD Ryzen Threadripper PRO 7965WX, Rust release build, single simulation thread. These are individual runs on a shared machine, not browser/device capacity claims. The `measure` example fires seven shells across 1,200 ticks:

| Scene | p95 tick | Maximum tick | Final material chunks |
| --- | --- | --- | --- |
| Barrier | 3.61 ms | 19.09 ms | 197 |
| Cantilever | 1.19 ms | 6.51 ms | 135 |
| Rock field | 0.14 ms | 0.66 ms | 33 |

The `resolution` example fires one shell at the editor rock for 120 ticks, first with strength raised to isolate rigid/stress cost, then with its normal breakable material:

| Initial cells | Intact p95 / peak | Breakable p95 / peak |
| --- | --- | --- |
| 5 | 0.010 / 0.080 ms | 0.074 / 0.342 ms |
| 20 | 0.023 / 0.160 ms | 0.090 / 0.562 ms |
| 80 | 0.059 / 0.495 ms | 0.253 / 3.302 ms |
| 320 | 0.194 / 1.931 ms | 0.787 / 18.282 ms |
| 1,280 | 0.765 / 10.153 ms | 4.283 / 180.318 ms |

All these runs reported zero failed stress solves. The highest-detail fracture spike still misses an interactive frame budget: topology changes rebuild sparse systems and collision geometry. One rigid body does not make thousands of colliders or repeated fracture solves free. Start with editor detail 0–2 and use adaptive local subdivision. The 2,048-cell authoring cap is not a performance promise. Run `cargo run --release -p sim --example resolution` on the actual target device; `impact` separately sweeps ammunition mass/speed. Native timestamps exclude rendering.

The browser smoke test checks startup and runtime exceptions, pause/step/queued shots, impact and shell removal, recoil displacement, fracture/cleanup, driving and braking, independent track input, rotational braking, normalized smooth zoom and its cursor anchor, rock chunk retention, the field manual, stress overlay, scene selection, and high-DPI rendering. Screenshots are saved to `target/screenshots/` for visual inspection.

After building and serving the browser bundle, launch an isolated Chrome instance:

```sh
google-chrome --headless=new --remote-debugging-port=9227 \
  --user-data-dir=/tmp/fracture-browser-test \
  --use-angle=swiftshader --enable-unsafe-swiftshader about:blank
```

Then run `node tools/browser-smoke.mjs` and `node tools/editor-smoke.mjs` from the repository root. The editor suite exercises real canvas input, browser file upload/download, and an isolated large triangle that acquires internal seams and breaks on impact. Reset restores its original single-cell geometry and runtime fracture leaves the saved draft unchanged. Screenshots are in the same directory. Node 22+ supplies WebSocket; no npm packages are needed. `SANDBOX_URL` and `CHROME_DEBUG_URL` override the local addresses. Software rendering is used for these checks.

**Third-party assets.** The interface uses unmodified Liberation Mono Bold under the SIL Open Font License in `crates/sandbox/assets/FONT-LICENSE`. Environment, objects, particles, and HUD geometry are generated from lines and shapes; font glyphs are rasterized. The vendored Macroquad browser loader's strict-mode fix is documented in `web/VENDOR.md`.

The workshop now supports authored stone grain points and staggered bricks, with editable E/Gc. Use **COMPARE VARIANTS** to run synchronized shell or tank trials and save reproducible experiment recipes. See [EXPERIMENTS.md](EXPERIMENTS.md); `node tools/comparison-smoke.mjs` uses the same browser verification setup and restores both original local-storage stores. The performance figures above describe earlier reference meshes, not a four-world browser benchmark.

Use **U** or **Choose Vehicle** to redeploy as a Scout Car, Regular Tank or Mortar without rebuilding the scene. The same picker appears after subsequent deaths. Opening the picker resumes a paused simulation so deployment can proceed; **R** still rebuilds the original scene.
