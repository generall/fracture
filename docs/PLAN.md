**Project plan: physics-driven tank destruction**

Proposed on 2026-09-12. This is the original starting design; the joint baseline has since been replaced with rigid chunks, a separate elastic stress graph, and adaptive subdivision. See [the current architecture](DESTRUCTION.md) and [measured results](SANDBOX.md). Performance numbers below are original targets, not results.

**Recommended foundation.** Use Rust and Rapier 2D for rigid-body dynamics, with a custom destruction layer and a Macroquad application for rendering, input, audio, and inspection controls. Build a destruction laboratory before adding tanks. Keep simulation independent of the application.

Existing engines cover collisions, motion, friction between objects, joints, and spatial queries. The distinctive work is deciding how structures transmit loads, where material fails, how geometry separates, and what that means for a machine's operation. We should build that layer while reusing the underlying physics engine.

**Engine choices.** These are suitability judgments for this project, not benchmark rankings.

| Option | Relevant capabilities | Decision |
| --- | --- | --- |
| [Rapier](https://rapier.rs/docs/) | Rust; 2D and 3D rigid bodies, joints, events, snapshots, optional determinism. | Use `rapier2d` directly inside the simulation crate. It lets us work independently of a particular game framework. |
| [Avian](https://github.com/avianphysics/avian) | Rust physics built around Bevy's entity-component system; modular internals, CCD, joints, and custom constraints. | A credible alternative if we decide to organize the whole simulation around Bevy. It still leaves material fracture to us. |
| [Box2D](https://box2d.org/documentation/) | Dedicated 2D rigid-body engine written in C17. | Credible alternative; Rust bindings and the C build introduce integration work without removing the custom destruction problem. |
| [NVIDIA Blast](https://nvidia-omniverse.github.io/PhysX/blast/index.html) | Destruction library with chunk hierarchies, splitting, and a separate stress extension. Core layers are independent of physics and rendering; the toolkit has a C++ API. | Study its architecture. Adopting it would require a Rust/browser integration investigation; it is not a ready-made Rust 2D simulation stack. |

[Macroquad](https://macroquad.rs/) supplies a small Rust API for shapes and a browser path using WebGL, as well as desktop support. That matches a physics laboratory particularly well. [Bevy](https://bevy.org/) offers an ECS and broader game-engine infrastructure; choose it if those become a concrete requirement. Keeping rendering separate preserves the simulation, but changing frameworks would still require application work.

Pin published dependency versions and commit a lockfile when scaffolding. Start with a single-threaded simulation and no GPU compute requirement. Rapier documents WASM support and an optional determinism feature, but the application must also behave deterministically. See its [setup guide](https://rapier.rs/docs/user_guides/rust/getting_started/).

**World assumptions.** Start with a true 2D horizontal simulation. Positions and rotations are continuous. An internal mesh partitions objects, but does not impose a movement or placement grid.

Use meters, kilograms, and seconds. Give objects an explicit effective depth so polygon area can be converted to volume and mass. Connections also need dimensions so their strength scales with geometry. These quantities describe an approximation to a horizontal slice; they do not make it a full 3D simulation.

Gravity has no component in the simulated plane. Implement ground resistance and tank traction explicitly; ordinary collision friction between visible objects does not supply friction against an unseen ground plane. Structural foundations are explicit anchors that can fail.

This supports ramming, cracks, separation, pushing debris, and planar vehicle articulation. It cannot directly simulate a roof falling downward, overturning, projectile height, or debris stacking vertically. An isometric drawing alone does not add those effects. If those are essential, revisit the choice before implementing vehicles and use 3D physics under an orthographic camera. That would be a substantial simulation change.

**Destruction model.** Represent an object with convex pieces and a graph of their connections. Pieces have geometry, material, mass, and inertia. Connections describe shared material or an actual seam such as mortar, a weld, or a mechanical attachment.

The first experiment uses a modest number of Rapier bodies with breakable joints. Joint reaction impulses provide estimates of connection loads; Rapier exposes these through [ImpulseJoint](https://docs.rs/rapier2d/latest/rapier2d/dynamics/struct.ImpulseJoint.html). Resolve linear loads into the connection's local normal and shear directions and account for torque when evaluating bending. Divide impulse by the actual solver interval only after confirming the selected Rapier version's accumulation semantics. A frame-wide collision-force magnitude is not a local stress tensor.

Start with brittle failure of seams. Compare estimated connection stresses against material-dependent limits, accounting for seam area and bending geometry. Remove a failed joint at a defined boundary between simulation steps. The released pieces already have physical velocities and colliders, and can hit other objects. This establishes a measurable baseline without immediately implementing an elastic-solid solver.

Joint forces are numerical estimates, sensitive to timestep, solver iterations, stiffness, and mass ratios. This prototype is an engineering approximation for a game, not validated material science. Initially, cracks follow the available seams and each piece remains rigid. Repeated weak impacts need not accumulate damage until an explicit model for cracking or plastic deformation exists.

For larger structures, investigate grouping connected pieces into compound rigid bodies and using a separate coarse structural graph to estimate internal loads. Only adopt this after comparing its failure behavior and runtime with the joint baseline. The graph solver must account for constraints, inertia, and load paths; spreading arbitrary damage values over neighbors would not establish material behavior. NVIDIA's [stress extension](https://nvidia-omniverse.github.io/PhysX/blast/docs/api/extensions/ext_stress.html) is useful precedent: it distributes loads across a bond graph with separate tension, compression, and shear limits.

On a compound-body split, recompute each component's mass, center of mass, and inertia. Inherit the parent's velocity field, including angular motion at each new center. Check linear and angular momentum conservation in an isolated split. Account for fracture work and any released elastic energy when those models are introduced; never invent a fragment explosion by adding arbitrary physical impulses. Preserve holes in collision geometry as well as in rendering.

**What replaces health points.** Failure follows changes to material and connectivity. A surviving fragment does not disappear because its parent was considered destroyed. A machine can remain mostly intact but lose a drive assembly or a turret mount. Later penetration can sever a represented connection or intersect a critical component. Functional rules translate those local physical changes into lost capabilities.

Material parameters include density, connection stiffness, tensile and shear strength, and eventually fracture energy and yield behavior. Persistent damage, when implemented, means broken seams, changed geometry, crack opening, or plastic strain. An object-wide durability counter is unnecessary. Strength values must scale with dimensions rather than the arbitrary number of pieces.

Begin with masonry joined by weaker mortar and a brittle panel with a narrow connecting section. Add wood with directional properties afterward. Steel should initially appear as rigid plates with breakable attachments; credible denting, bending, and tearing require a plasticity model. Merely giving the brittle model a higher threshold would not reproduce steel's behavior.

**Where FEM fits.** A coarse lattice or beam model is the next candidate when we need elastic load transmission and bending. A central-force spring network alone is insufficient for general bending and shear. Evaluate beams with angular degrees of freedom or a small triangle mesh with an energy-based deformation model.

XPBD is a possible numerical method for compliant constraints and elastic energies; it is not itself a fracture or plasticity model. The [original XPBD paper](https://matthias-research.github.io/pages/publications/XPBD.pdf) describes the compliance formulation. Material laws, mesh quality, tearing, contact coupling, and numerical validation remain our responsibility.

Full dynamic FEM would add substantial work around stiff materials, time integration, changing topology, and collision geometry. GPU time saved by simple graphics does not automatically become CPU time for those calculations. Introduce additional simulation detail only after demonstrating a specific missing behavior, such as a beam permanently bending before it separates. A stress estimate on a rigid cluster can select a fracture location but cannot produce visible deformation by itself.

**Impacts and projectiles.** Initially use physical impactors with CCD for repeatable experiments. Rapier's [CCD](https://rapier.rs/docs/user_guides/rust/rigid_body_ccd/) helps detect collisions along fast motion; it does not implement armor penetration.

Later, use swept projectile queries and a dedicated impact model that accounts for thickness, impact angle, available kinetic energy, and work consumed by penetration or fracture. Remaining energy determines whether and how the projectile continues. Give that impact model clear ownership of its impulses so Rapier does not also resolve the same hit as an independent full collision. Keep recoil and momentum accounting explicit. Treat this as a gameplay approximation, not a promise of predictive ballistics.

**Application structure.** The initial Cargo workspace needs two crates:

| Crate | Responsibilities |
| --- | --- |
| `sim` | World state, Rapier integration, materials, connection graph, fracture, tick-indexed commands, events, and reproducible scenarios. Headless tests and benchmarks live here. |
| `sandbox` | Macroquad window, controls, camera, filled polygons and outlines, debug overlays, sound, and presentation interpolation. |

The application supplies commands to `sim`, advances fixed ticks, and reads state and events for presentation. The simulation has no dependency on frame time, rendering, browser APIs, or audio. Material definitions and scene seeds are data. Stable IDs and explicit event ordering support reproducibility.

Start at 120 simulation ticks per second and interpolate presentation independently. Compare 60 and 120 Hz in controlled experiments. Bound catch-up work after long pauses; make overload visible in the laboratory rather than silently increasing the physics timestep. Slow motion changes how simulated time advances, not material strength.

The visual design uses opaque flat fills, clean outlines, visible fracture gaps, and a restrained material palette. Contact vectors, connection loads, anchors, and centers of mass can be shown in debug mode. Brief impact flashes, trails, and procedural sound help communicate force without obscuring the physical result. Dust can be a non-colliding visual effect; chunks large enough to affect gameplay retain physical representation.

**Milestones and evidence to proceed.**

1. **Motion laboratory and browser smoke test.** Create the workspace, fixed-step loop, shape renderer, camera, reset, pause, single-step, and projectile launcher. Run the same small scene natively and in a browser. Verify free movement, recoil, collisions, and ground resistance. This makes browser compatibility an early fact rather than a late porting assumption.
2. **First satisfying fracture.** Add an anchored masonry barrier and a panel with a narrow neck, each with a few dozen pieces and breakable connections. Expose impact mass, speed, and aim. A small impact leaves the structure intact; a larger one opens a persistent gap; changing impact location changes the response; detached chunks can cause secondary failures. Include at least one leverage test where failure occurs away from contact. Add slow motion and load visualization.
3. **Material behavior and numerical checks.** Compare connection types, dimensions, and grain direction. Test unsupported versus anchored structures and a constrained beam under a known load. Repeat scenes across timesteps, iteration counts, and mesh resolutions. Record unstable or non-convergent cases. Decide whether a custom graph solver or deformable model is needed based on missing behavior and measured cost.
4. **Tank playground.** Add a hull, turret, and separate drive assemblies. Apply traction at left/right contact regions; avoid simulating every tread link initially. Detachment removes the corresponding force path. Demonstrate asymmetric steering after losing a drive, turret detachment, recoil, and the ability to breach and drive through a barrier. Add approximate penetration after basic impacts behave consistently.
5. **Scale and presentation.** Profile dense rubble and simultaneous impacts on a named desktop browser and native build. Introduce sleeping, simpler fragment colliders, and clustered structures if justified. Add sound and visual feedback early enough to evaluate enjoyment. Preserve gameplay-significant rubble when reducing cost.
6. **Multiplayer prototype.** Run the same simulation on an authoritative Rust server. Replicate inputs, stable object IDs, topology changes, and state snapshots; interpolate remote objects and investigate prediction for the local vehicle. Reserve reliable delivery for structural changes. Cosmetic effects can be client-local. Networking does not begin with an assumption that every client can reproduce every fracture bit-for-bit.

**Validation and budgets.** The first benchmark target is a scene with roughly 200 moving fragments and 1,000 connections at 120 Hz. On a recorded desktop browser/device, aim for a 95th-percentile combined physics/fracture tick below 4 ms and inspect worst-case stalls. These are provisional targets and must be revised using measurements; they are not engine capacity claims. Measure active bodies, contacts, joints, allocations, and fracture spikes, not only average frames per second.

The meaningful checks are: no spontaneous fracture at rest; reaction loads consistent with simple statics where applicable; different results for different impact locations and seam dimensions; no tunneling within the documented projectile envelope; no NaNs or persistent explosions of energy; sensible behavior under timestep and mesh refinement; correct collision geometry after separation; and reproducible outcomes from a scene seed and command log on the same build. Any later cross-platform determinism claim requires native/WASM comparison tests of the complete simulation.

The first playable deliverable is a shape-rendered destruction sandbox in which aiming at different parts of a structure produces understandable, persistent, and enjoyable physical consequences. It is the experiment that determines how much additional material simulation this game needs.
