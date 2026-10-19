# Controller reset, 18 October 2026

The v41–79 experiments did not establish a dominant challenger. More local rules
and configuration searches are suspended. The current failure is composition:
several independent behaviours overwrite tracks and goals in one decision, and
partial routes are treated as successful navigation.

The replacement is opt-in (`task_control`) so frozen opponents remain unchanged.
It keeps the existing public-map and delayed human-observation boundary. It does
not change physics, random vehicle selection, reaction time, or promotion rules.

The control pipeline is:

1. Update remembered geometry from delayed observations.
2. Advance a persistent mission (travel, breach, enter, or scout screening).
3. Select exactly one task, with persistent traffic/recovery interruptions.
4. Render that task into tracks once.
5. Independently aim/fire using visible targets or the task's breach target.

Travel caches a complete route until invalidated. An unreachable route is an
explicit failure, never permission to drive toward its partial endpoint. Breach
progress is measured from visible geometry, with bounded attempts and another
approach after failure. Scouts cannot select a wall-clearing task. Vehicles own
their mission rather than continuously chasing a teammate's position.

Before another tournament, require real-physics behaviour tests for both river
directions, different headings during a teammate pass, and base breach/entry.
Record task choice, route outcome and fire refusal. Navigation fixtures and
stationary-opponent probes are diagnostic evidence, not league wins. Promotions
remain at zero until the existing native Deck gate passes.

## Evidence before competitive evaluation

The new physics contracts cover both river directions for scouts, tanks and
mortars, teammate passing with three different scout headings, and tank breach
and capture on both base layouts. They pass. Incomplete routes produce zero
tracks without repeated planning; target appearance still waits 300 ms.

A complete diagnostic team test also passes: one tank and one scout start at
their normal River Valley spawns, authored fixed defenses remain active, and
opposing vehicles remain stationary. The corrected controller captured for
Blue in 143.5 seconds and Red in 228.9 seconds. Fixed vehicle choices and stationary opponents make this a
behaviour contract, not competitive evidence. Run it with:

```sh
FRACTURE_SILENT=1 cargo test -p sim ai::task::tests::team_travels -- --ignored --nocapture
```

The contracts exposed three specific defects during implementation:

- Lookahead skipped route cells without consuming them; an old cell could later
  pull a vehicle backwards. Clear shortcuts now consume their route prefix.
- Rounding a valid spawn pose to a blocked grid centre disconnected the tank
  from the graph. Continuously checked connections from the actual pose fix it
  without reducing terrain or obstacle clearance.
- Total visible wall area increased when a successful shot revealed inner
  walls. Breach progress now compares occupancy at the same visible positions.

V80 was rejected: native Deck navigation passed 18/19, with the mortar unable
to leave its garage, and a repeated full team contract found another Red-side
deadlock. Its one competitive host smoke round drew 0–0 on flags. V81 adds
explicit driver outcomes, checked straight departure edges for narrow starts,
visible-geometry local driving with spatial bins, and a short standoff retreat
inside a breach. These changes pass all eight task contracts and the separately
run full team contract. The workspace passes 426 tests, with seven ignored;
strict Clippy and WASM checks also pass. V81 passed all 19 native Deck navigation
probes, all retaining 100 HP. The frozen agent and actual remote probe binary
hashes are pinned in
`validation/ai-task-control-deck-v81-2026-10-18.json.gz`. The separate Deck viewer
now runs Blue v81 against frozen Red v11. Competitive dominance is still unproven.

One native four-minute 2v2 smoke match has completed (seed 42): 0–0 captures,
7–7 enemy deaths. Challenger closest enemy-flag distance was 14.47m, incumbent
31.81m. The challenger never selected its wall-clearing phase in this match;
reaching the staging point under live combat is the next integration problem.
Do not tune breach damage based on this round. Native decision time was 8.26ms
p95 / 14.99ms p99, with 21.17ms maximum. The viewer returned to roughly 52 FPS
after the concurrent background match ended; no claim of fixed 60 FPS or solved
freezes is warranted. Full native smoke proof:
`validation/ai-task-smoke-deck-v81-2026-10-18.json.gz`.

Further work must extend complete behaviour contracts rather than return to
parameter sweeps. Include identical-chassis teammates while one is firing at a
wall, and independent scout missions rather than sending every chassis down
the same approach route. Preserve human-visible information and the reaction
queue when improving fire control.

The development configuration is `tools/ai/task-controller.json`. Legacy experiment flags do not stack movement behaviours in
this controller. `siege_tactics=false` selects navigation-only objectives for
the existing driving probes. Recordings include the selected task, decision
reason, complete/unreachable route status, local driver outcome and reason for withholding fire.

## Integration findings after the reset

V82 and v83 each drew all four host comparisons against v11 on flags. They are
rejected as promotions. V83's 27:23 kills do not change that result. The native
evolution ledger remains **0/5**; v11 remains frozen.

The task executive now has independent scout interception stations, bounded
combat interruptions, delayed sight-based motion estimates, and complementary
heavy-vehicle routes. These are mission choices above the driver, rather than
additional functions that replace its track commands.

A controlled moving-target probe found a lower-level interface defect: holding
an aim point for 100 ms makes the turret move in full-speed bursts. Its reticle
never settled enough to fire at a crossing scout. Workers now return twelve
planned aim samples from the same delayed decision. The harness and observer
apply these at the normal 120 Hz physics rate, with fire requested only at the
last sample. This adds no perception updates or faster reactions; sensing stays
at 10 Hz with at least 300 ms delay. Old workers keep their original held input.
The frozen v83 crossing-target tank probe changed from zero shots to one shot
and a kill at 1.94 s. The scout/mortar crossing cases still have limitations;
this is not evidence of competitive dominance.

V84's full guarded perimeter contract passes with scheduled inputs: Blue
captures at 177.6 s and Red at 210.8 s. It uses a fixed tank/scout pair and
stationary opposing vehicles, with authored defenses active. All 18 ordinary
task contracts also pass. Route checkpoints are chosen before a side route
starts retreating from the objective. Aim follows the next travel waypoint;
wall clearing targets the actual visible fragment. A proposed shortcut that
aimed through the wall at the flag failed the Red breach contract and was
removed before freezing.

In frozen v84 the scout can spend about 20 seconds trying to
reach a yielding bay outside the spawn garage. Passing has a timeout, so the
team does proceed, but the local car planner's arrival prediction needs a
dedicated garage-departure contract. Do not hide this by merely changing a
timeout or counting it as a successful optimization. The unfrozen correction
predicts measured own speed, acceleration, yaw and braking, and can stop during
an arrival rollout. Reverse arrivals use the correct heading. Its new real
garage contract departs in 15.7s Blue / 12.9s Red, before the passing timeout.
All 19 host driving cases pass with 100 HP, and the full guarded contract passes
at 183.6s Blue / 174.0s Red. This has no competitive verdict yet.

V84's four mirrored host trials also drew on flags (22:16 kills). The Deck was
then switched to the user's requested recorded human 1v2 against frozen v11.
Do not interrupt that session for another experiment.

Further strategy changes require complete behavior evidence before a frozen
comparison. Preserve the same seeds, independent random vehicle choices,
normal weapons/physics, information boundary and native promotion gate.
