# Physics performance on Steam Deck

The 2026-09-27 investigation compares CPU stage costs, Rapier worker pools, and
optimizations of physics bookkeeping, visibility and AI queries. Normal game builds keep Rapier
single-threaded. The optional `sim/parallel` feature is retained for experiments;
it is not enabled by the Linux or browser packaging scripts.

## Workloads and measurement

`crates/sim/examples/physics_benchmark.rs` runs four fixed-input workloads:

- River Valley: the bundled `levels/dota-level.json`, firing HE at nearby cover.
- Classic Tanks 2: `levels/classic-tanks-2.json`, with the same targeting rule.
- Masonry: repeated HE fire into the destruction test fixture.
- Barrel: continuous driving into the barrel/rubble crushing fixture.

The first-pass runs each start a fresh world and simulate 1,200 ticks (10 seconds
of game time). Follow-up headless runs use 2,400 ticks.
Map enemies stay static so flag capture cannot end the workload early. Targets
are chosen once from the original geometry. Map construction, serialization and
fingerprinting are outside the timed `World::step` call. The first simulation
tick remains in the samples. These are destruction workloads, not idle scenes.

Hardware: Steam Deck, AMD Custom APU 0932, four cores/eight hardware threads,
AC connected, existing `powersave` governor. The installed game was suspended
only during benchmark batches and resumed afterwards. No power or clock settings
were changed. Runs are headless and as-fast-as-possible: they measure physics CPU
cost, not FPS, rendering, GPU load or battery life under actual gameplay.

The serial baseline predates the two bookkeeping changes below. Dedicated pools
of one, two and four workers were each run three times, with changing run order.
The default global pool was also tested with two and four workers, twice each.
The optimized serial build was run three times, with another baseline run between
optimized runs. The baseline therefore has four samples per workload.

`physics-profile` enables top-level stage timers and nested timers for chunk
rebuilding and structural system assembly/solving. Nested values must not be
added to the top-level stage totals. Normal builds contain neither these clocks
nor profiling storage. Instrumentation is identical across timed variants.

## First CPU pass

Ground resistance now rejects fixed and sleeping bodies before inserting them
into its per-substep visited set. Moving bodies still receive exactly the same
friction operations, in the same order, once per body.

Fracture processing first identifies loaded chunks, then collects cell members
only for those chunks. It avoids allocating member vectors for untouched map
objects. Cell ordering, bond ordering, force accumulation, crack selection,
solver tolerances, four microsteps, and the 120 Hz tick remain unchanged.

All 61 output fingerprints from `regression_snapshot` matched before and after
these changes. Final-state fingerprints also matched across every benchmark
variant on the Deck. The full workspace tests and the simulation tests with
profiling/parallel enabled passed. These checks establish repeatability for the
tested scenarios and toolchain, not a universal cross-version determinism claim.

## First-pass measurements

Values below are medians of the repeated runs. CPU-time reductions are not FPS gains.

| Workload | Baseline mean | Optimized mean | CPU time saved | Baseline p99 | Optimized p99 |
| --- | ---: | ---: | ---: | ---: | ---: |
| River | 5.351 ms | 4.183 ms | 21.8% | 14.519 ms | 12.391 ms |
| Classic | 3.538 ms | 2.819 ms | 20.3% | 13.286 ms | 11.818 ms |
| Masonry | 4.471 ms | 4.345 ms | 2.8% | 12.931 ms | 12.690 ms |
| Barrel | 0.733 ms | 0.717 ms | 2.2% | 1.844 ms | 1.824 ms |

| Rapier configuration | River mean | Classic mean | Masonry mean | Barrel mean |
| --- | ---: | ---: | ---: | ---: |
| serial | 5.351 ms | 3.538 ms | 4.471 ms | 0.733 ms |
| dedicated1 | 7.198 ms | 5.227 ms | 7.492 ms | 1.239 ms |
| dedicated2 | 6.649 ms | 5.166 ms | 6.388 ms | 1.217 ms |
| dedicated4 | 6.749 ms | 5.151 ms | 5.313 ms | 1.233 ms |
| global2 | 5.688 ms | 3.824 ms | 4.858 ms | 0.855 ms |
| global4 | 5.437 ms | 3.855 ms | 4.769 ms | 0.865 ms |

The global pool reduces the overhead of the dedicated-pool approach, but neither
configuration produced a useful overall improvement over serial execution on these
workloads. Parallelism stays opt-in; no physics tick rate or solver quality was reduced.

Baseline River Valley spends approximately 18% of measured time in Rapier.
Much of the remainder is custom fracture/contact/ground processing. Accelerating
Rapier alone on a GPU would leave those costs in place. A GPU port is deferred;
the current evidence favors reducing CPU bookkeeping before changing engines.

The p99 still exceeds the 8.33 ms physics-tick budget in the destruction-heavy
map scenarios. This improves average cost and spikes; it does not eliminate all
hitches. These first-pass headless figures do not predict frame-rate gains; the follow-up
below measures complete native frames.

Raw measurements, stage breakdowns, worst ticks, workload stats and final-state
fingerprints: [all 80 measurements](validation/physics-deck-2026-09-27.jsonl).

## Follow-up for wall-heavy gameplay

The follow-up measures the complete native game because the headless workloads
do not include visibility, fog, rendering or an active enemy. The initial valid
45-second Classic Tanks 2 capture averaged 44 FPS despite the faster fracture
code. Fog alone averaged 6.87 ms per frame, and later sustained captures exposed
expensive enemy retreat checks. Average physics timing alone was insufficient.

The implementation now:

- Indexes incident bonds by stable cell ID and gets chunk members from existing
  collider ownership, preserving ascending member/bond order.
- Rebuilds only affected chunks after cracks, subdivision and laser cuts. Support
  endpoints participate in cache invalidation and visible-edge updates.
- Caches the first active cell per body for floor friction, invalidating after
  topology changes or added cells. Rapier's original body-update order is retained.
- Stores contact motion snapshots in sorted contiguous storage, preserving the
  order of kinetic-energy sums while avoiding per-body tree allocations.
- Rejects distant fog polygons and AI sight blockers using conservative bounds
  before exact geometry tests. Sight area accumulation uses dense scratch arrays;
  fog lineage uses sorted vectors and lookup-only hashing for sparse network IDs.
- Caches resolved grass occluders instead of repeating terrain subtraction for
  every sight query. Source comparison preserves paint precedence after edits
  and replica map changes.
- Skips redundant navigation-cell projections and tests retreat/respawn overlap
  only for obstacles near the complete tank footprint.

The physics tick rate, four microsteps, solver iterations, fracture work rules,
debris lifetimes, driving parameters and controller behavior remain unchanged.
All 61 replay fingerprints match the pre-follow-up build, as do the four longer
2,400-tick destruction workloads. New reference tests compare indexed membership,
local/full chunk rebuilding, ray visibility and rotated tank clearance against
their full-scan equivalents. A terrain-cache test also covers overlapping paint,
reordering, same-size edits, clearing, and repeated reads.

### Follow-up headless measurements

Two final serial runs on the Deck, 2,400 ticks per workload. The comparison is
against the previous first-pass optimization run at the same 2,400-tick length,
not the shorter first-pass table above. Mean is averaged across the final pair;
p99 is shown as a range. All four final-state fingerprints match.

| Workload | Previous mean | Final mean | Final p99 range |
| --- | ---: | ---: | ---: |
| River Valley | 4.084 ms | 2.349 ms | 7.064–8.350 ms |
| Classic Tanks 2 | 2.872 ms | 1.536 ms | 4.758–5.081 ms |

The final serial Classic workload uses about 47% less physics CPU time than the
previous pass. These figures exclude rendering and active enemy AI. Raw data
includes the previous run, two intermediate locality runs and two final runs:
[20 follow-up workloads](validation/physics-deck-followup-2026-09-27.jsonl).

### Native gameplay measurements

Foreground native game at 1280×800 in SteamOS Desktop Mode, display at 90 Hz,
AC power and unchanged governor, three-second warm-up followed by two minutes
of driving/firing with active enemy AI. Music and effects were muted using the
process override; saved settings were untouched.

| Map / run | Average FPS | p95 frame | p99 frame | Frames >50 ms | Simulation / wall |
| --- | ---: | ---: | ---: | ---: | ---: |
| Classic Tanks 2 / 1 | 74.7 | 20.2 ms | 29.0 ms | 13 / 8,968 | 120.008 / 120.008 s |
| Classic Tanks 2 / 2 | 75.6 | 20.6 ms | 31.0 ms | 3 / 9,073 | 120.000 / 120.005 s |
| River Valley / final | 65.4 | 28.9 ms | 73.8 ms | 152 / 7,845 | 119.442 / 120.008 s |

Both Classic runs broke over 1,000 bonds and retained roughly 5,900 active cells.
They use the final physics, fog and AI optimizations; the later grass cache does
not alter this map's empty terrain. This is sustained destruction, not an idle
FPS sample. The second run's three >50 ms frames coincide with bursts of Rapier,
contact and fracture processing (maximum 59.8 ms); the first run peaked at
90.8 ms. Occasional destruction hitches remain, so this is not a locked-60 claim.
River exposed repeated subtraction of static grass terrain: before caching, its
120-second run averaged 29.1 FPS and simulated only 116.3 seconds. The final cache
reduced mean player-vision CPU time from 11.08 to 1.99 ms per frame. Different
frame schedules change the destruction history, so this is not a controlled
FPS multiplier. The final run broke 1,235 bonds, but its larger destruction
bursts still caused 152 frames over 50 ms, a 149.4 ms maximum and 0.57 seconds of
catch-up loss. River's average is substantially better, but it is less consistent
than Classic; follow-up work should target burst contact/fracture/Rapier cost.

Gaming Mode, battery life and longer human play sessions were not measured here.
The final normal package is deployed at `~/Games/fracture`; the running process
hash was verified as
`58a637c77ed47dc8ffbf5664e1ee252a99a6e10c08cc0f49d7ebf17248f1bc7f`.
The prior executable is retained as `game.before-deck-performance`.

[Native summaries and stage percentiles](validation/gameplay-deck-2026-09-27.json)
and [compressed raw frame samples](validation/gameplay-deck-2026-09-27.frames.json.gz)
include the initial valid Classic capture, both final Classic runs, and River
before/after grass caching. Decode the latter with `gzip -dc`.

Validation: 274 workspace tests, 214 simulation tests with parallel/profiling
features, strict Clippy (with the repository's three existing lint allowances),
61 unchanged replay fingerprints, Linux/WASM builds, and the game/Deck browser
suites passed. Internal native and browser tests were silent.

## Reproduce

### Native gameplay profiling

The opt-in `sandbox/performance-test` feature runs the real game loop, including
an attacking opponent, driving into walls, firing, fog, rendering and audio
updates. Normal packages omit the fixture and its frame timers. It does not
change the saved map or mixer settings.

```sh
cargo build --release --locked -p game --features fracture_client/performance-test
FRACTURE_SILENT=1 FRACTURE_DATA_DIR=/tmp/fracture-perf-save \
  FRACTURE_PERF_SCENARIO=classic FRACTURE_PERF_SECONDS=120 \
  FRACTURE_PERF_OUTPUT=/tmp/fracture-classic-frames.json \
  target/release/game --deck
```

Use `river` for the other bundled map. Keep the game window in the foreground
and the display powered on throughout the run. On KDE, hold ScreenSaver and
PowerManagement D-Bus inhibit cookies for the test lifetime and release them
afterwards; do not change the player's permanent power settings. A powered-off
Deck display throttles this desktop workload to 1 FPS and is not valid gameplay
evidence. Early captures caught this condition and were excluded.

The first three seconds are warm-up for window activation and startup. Reports
include every subsequent frame interval, CPU stage times, simulation progress,
physics stage totals and final destruction statistics. Rendering submission time
does not measure GPU execution or presentation waits; the complete frame interval
includes those waits. Compare simulated seconds with wall seconds to detect time
discarded by the catch-up limit. Different frame schedules can shift scripted
shots and produce different destruction histories, so frame runs are gameplay
workloads, not identical deterministic traces.

### Headless physics profiling

Build the serial profiler:

```sh
cargo build --release --locked -p sim --example physics_benchmark --features physics-profile
cp target/release/examples/physics_benchmark /tmp/fracture-serial
/tmp/fracture-serial --ticks 1200
```

Build the optional parallel variant and compare both pool types:

```sh
cargo build --release --locked -p sim --example physics_benchmark --features physics-profile,parallel
cp target/release/examples/physics_benchmark /tmp/fracture-parallel
/tmp/fracture-parallel --threads 2 --ticks 1200
RAYON_NUM_THREADS=2 /tmp/fracture-parallel --global-pool --threads 2 --ticks 1200
```

Repeat with `--threads 4` (and `RAYON_NUM_THREADS=4` for a global pool). Each output
line is JSON with mean/p50/p95/p99/max tick time, ticks above the 8.33 ms budget,
accumulated stage times, the slowest tick's stage breakdown and simulation stats.
`--scenario river|classic|masonry|barrel` selects a workload. `--verify` hashes
state and events after every tick; hashing is excluded from tick timers but can
affect cache and thermal conditions, so use it for correctness comparisons
separately from performance runs. Without it, the final state is fingerprinted.

To compare simulation behavior across source changes, save the output of
`cargo run --release --locked -p sim --example regression_snapshot` before and
after editing and compare the files exactly. Use the same compiler/build profile.
