**Rigid chunks with adaptive fracture**

This document describes destructible objects. Deformable frames now use a separate [permanent deformation path](PLASTICITY.md): they stay connected under impact and do not enter stress-triggered fracture or adaptive subdivision. The [laser](AMMUNITION.md) explicitly subdivides and removes cells in either object mode, without force or fracture-work loads.

The simulation separates visible motion from internal stress. Connected material is a single rigid body. A separate elastic graph determines how loads reach seams and supports. Cracks change connectivity; newly disconnected components become rigid bodies. This replaces the original network of soft Rapier joints, whose numerical compliance grew with mesh detail and made rock behave like rubber.

| Layer | Responsibility |
| --- | --- |
| Rapier | Chunk motion, mass/inertia, contacts, friction, sleeping and CCD |
| Structural graph | Virtual elastic displacement and equilibrated seam forces under applied loads |
| Fracture | Stress thresholds, available crack work, connectivity and rigid-body splitting |
| Adaptive geometry | Local triangle subdivision while preserving surfaces, seams, supports and motion |
| Cleanup | Remove sufficiently small disconnected chunks after a delay |

**Rigid motion.** Each active cell retains a triangle and a local pose within its chunk. All cells in a connected chunk use the same body transform. Internal cells cannot collide with one another, and no soft joint chain can distort the shape. A component attached to a fixed support is fixed; once disconnected, it moves freely. Splitting recomputes mass, center of mass and inertia, and inherits the parent's velocity at each new center plus its angular velocity. There are no artificial outward kicks. Tests check mass, linear/angular momentum and kinetic energy before and after refinement and splitting.

**Internal stress.** Each cell has three virtual degrees of freedom: two translations and one rotation. Neighboring cells resist relative displacement at their shared face and relative rotation, using EA/L, GA/L and EI/L stiffnesses. The face has its actual width and material depth; L is the normal distance between cell centers. Off-center forces contribute torque. Fixed supports constrain virtual displacement. Free chunks subtract mass-weighted rigid acceleration and angular acceleration, so the solve measures the load required to keep their material moving together.

This is a small-strain interface/beam network, not a continuum triangle FEM formulation. Elastic displacements never deform collision or rendered geometry. Deformable objects use a separate projection of yielded rest offsets to update their permanent shape. It transfers axial, shear and bending loads to distant supports and is checked against simple statics. Actual elastic waves and elastic plate recovery would need a different integration model.

The stiffness matrix and its sparse LDLᵀ factorization are cached in the chunk's local frame. Deterministic minimum-degree ordering at the cell level limits fill. Double precision, diagonal scaling, residual correction and a bounded iterative fallback handle numerical conditioning. A solve whose force/moment residual fails tolerance cannot initiate cracks. The general sparse-factorization and ordering principles are described in [Davis's LDL documentation](https://github.com/DrTimothyAldenDavis/SuiteSparse/blob/dev/LDL/Source/ldl.c); this implementation builds its fill pattern from elimination cliques in Rust.

**Failure and work.** Normal force, shear force and moment are converted to seam stress using its section area and bending section modulus. Failure requires both an overload and enough available work for Gc × face area × grain-cohesion multiplier. The most overstressed affordable seam is removed, then stress is recalculated before another crack is chosen. This avoids breaking many seams from a stress field that the first crack has already relieved. Each load-processing pass permits up to eight such rounds per loaded chunk.

Shells and pressure pulses carry finite positive-work budgets. Crack work is deducted first; the remaining budget caps the kinetic-energy increase from the applied point impulse, including rotation. A source retains its remaining budget when a cell is subdivided or a body splits. Contact impulses have already been applied by Rapier: they are only used to estimate structural load, never applied a second time. Contact crack budgets are bounded by estimated dissipated work and, collectively, by the actual kinetic-energy loss during that collision substep. No object has a health counter and repeated weak hits do not accumulate hidden damage.

These are **energy-bounded brittle cracks**, not a full cohesive traction/separation law. Crack cost is paid from incoming or already dissipated work; there is no gradual crack opening or elastic energy release. The reported elastic energy is the virtual solve's diagnostic estimate, not stored energy injected into fragment motion. Contact dissipation is allocated approximately. A static force without an available work source cannot grow cracks in this model. Impact forces use impulse divided by the microstep, so dynamic fracture remains timestep dependent.

The illustrative bulk values are:

| Material | Young's modulus | Gc |
| --- | --- | --- |
| Rock | 40 GPa | 60 J/m² |
| Panel / plate | 8 GPa | 120 J/m² |
| Masonry | 15 GPa | 30 J/m² |

Poisson's ratio is 0.2. These are defaults; object assets can override E and Gc in the editor, along with density, thickness, friction and stress limits. Cohesion scales internal-grain strength and crack work. These are sandbox materials, not calibrated engineering data.

**Adaptive subdivision.** A sufficiently strong, energetic load can split a triangle of at least 0.075 m² into four equal-area children. This works inside a whole object, inside a connected fragment, or on an isolated triangle. New internal seams begin intact; subdivision alone is not damage. External interfaces are clipped onto the child edges with their original total face width. Existing cracks stay broken, supports retain their attachment, and impact locations and ongoing pressure pulses are mapped to the appropriate child.

The trigger estimates local impulse stress against half the material's cohesive tensile limit and requires enough source work for a candidate internal face. It is a refinement heuristic; the structural solve still decides failure. One impact/pulse can refine each affected lineage by at most one level. Later impacts can refine it again. Per pass, at most eight cells subdivide. Playable levels allow up to 8,192 additional cell records from runtime subdivision, including retired and cleared children, separately from authored map geometry. Material experiments retain the original 8,192 total-record cap for replay compatibility. Individual lineages have at most eight runtime generations. Rebuilding resets these guards. H shows counts and a message if a limit was reached. These guards can stop further subdivision even when a surviving triangle is still large.

**Cleanup.** The combined area of a disconnected, unanchored chunk must be strictly below 0.075 m² to start its timer. It fades during its final second and disappears after 3.0–3.9 simulation seconds. A small cell inside a large chunk stays. A large isolated triangle stays until later damage breaks it smaller. Cleanup intentionally removes mass and residual motion; it is a game rule, separate from fracture. Children have fresh stable IDs and retain an original-cell identifier for diagnostics; retired IDs are not reused.

**Level debris budget.** Playable levels target at most 8,192 active pieces. Every 30 ticks (0.25 seconds), excess pieces are reclaimed from already-disposable dynamic chunks outside conservative sight-radius circles plus an 8 m margin. All tanks (including tanks awaiting respawn), both flag lookouts and owned radar stations protect nearby debris. Candidates are ordered by total chunk area, then distance beyond the protected areas, then stable body handle. Cleanup removes whole chunks and their physics bodies/colliders; intact structures, fixed supports, bridge decks and connected sections are protected. New wrecks are never suppressed, and the budget stays soft when no safe candidates remain. Stable retired IDs remain for lineage and replication but consume no active-piece allowance. Material experiment cleanup and the separate subdivision record guard are unchanged.

**Remaining limitations.** Cracks still follow triangular edges, including the new midpoint subdivisions; they are not impact-aligned cuts. The exact fracture pattern is mesh dependent even though rigid shape, mass and splitting conservation are verified across detail levels. Only modeled impact and contact loads feed the graph; floor drag acts on whole chunks, and sustained centrifugal stress is not modeled. The graph ignores transient stress-wave propagation and has no fatigue, crack closure/contact within an otherwise connected chunk, or physical vertical collapse. Full cohesive opening, impact-aligned cutting and mesh-convergent continuum stress are subsequent physics work. The architecture allows those to evolve without replacing Rapier's rigid-body/contact layer.

**Authored grains and experiments.** Stone/Voronoi points and staggered brick dimensions define grain boundaries independently of solver refinement. Shared short faces are collapsed before triangulation to avoid nearly singular hinges, while preserving the outline and area. Cohesion scales grain interiors; runtime refinement inherits grain identity. See [EDITOR.md](EDITOR.md) and [EXPERIMENTS.md](EXPERIMENTS.md).

**Object response components**

Authored objects can carry separate shell/blast and contact resistance multipliers, opposed-contact crush behavior, and a one-shot damage-triggered explosive. Multipliers scale seam strength and required crack work without adding force or replenishing energy. Contact refinement can be disabled independently of weapon refinement. Colors and object/fuse identities survive subdivision. Explosive vessels are consumed before casting the same occluded pressure rays used by shells; a vessel ignites on internal fracture in destructible mode or accepted permanent deformation in deformable mode. See [OBJECT_LIBRARY.md](OBJECT_LIBRARY.md) for profiles and parameters.
