# AI match arena

## Recorded human challenge

Run `fracture-ai-arena.sh --deck --play-v11 --seed 42 --record-dir ~/.local/share/fracture-ai-challenge`
to play Blue alone against two frozen Red v11 workers on River Valley. The
Deck desktop shortcut is **Fracture — You vs 2 V11 bots**. Select a vehicle
with left/right and A; the four-minute timer starts with that choice. Movement,
aiming and special controls use the normal game mappings, fog and camera.
Respawning takes ten seconds and allows another vehicle choice. Menu pauses;
A starts another round after the result.

Each recording prefix has `.challenge.json` for the roster and frozen opponent
manifest, `.inputs.jsonl` for all three applied controls at 120 Hz,
`.decisions.jsonl` for restricted worker observations/decisions and the human's
visible view at 10 Hz, `.events.jsonl`, and `.summary.json`. `.frames` and
`.vision` store the separate full-field review. That observer data never feeds
the player's view or either worker. Human challenges are diagnostic evidence,
not AI-league promotion matches.

The native silent smoke exercised vehicle selection, driving and firing. It
recorded 1,235 physics ticks, three players, two v11 decision streams and an
8.34 m human displacement. The partial smoke is separate from the user's round.

## Steam Deck spectator

The separate `fracture-ai-arena` native binary runs **2v2** and shows the entire battlefield,
with B1/B2 and R1/R2 markers, current goals, health, kills and stall warnings. Four independently controlled AIs
run locally on the Deck in their frozen worker executables and still receive
only their own human-visible observations. Observer visibility does not change
their knowledge. It opens River Valley; switch to Classic Tanks with Y. Blue and red translucent
masks show the union of each team's current vision, including allied defensive
guns, flags and mines. Their overlap combines both tints. Paused replay uses the
vision recorded at that time. The observer still sees the full battlefield.

Build a standalone package with the two frozen versions to display:

```sh
FRACTURE_SILENT=1 bash tools/build-ai-arena.sh target/ai-arena/versions/v40 target/ai-arena/versions/v11
```

The output is `target/fracture-ai-arena-linux-x86_64.tar.gz`. Extract it and run
`fracture-ai-arena.sh --deck`. On the Deck it is installed separately at
`~/Games/fracture-ai-arena/`, with a **Fracture AI Battlefield** desktop launcher.
The viewer is silent and uses its own recording directory, with no player mixer
or map settings. The launcher holds temporary display/idle inhibitors only
while the viewer is open. On Deck the launcher disables display sync for this
process to avoid the compositor throttling swaps to one frame per second.

| Deck | Keyboard | Action |
| --- | --- | --- |
| A / Menu | Space | Pause/resume; play the recording after the round ends |
| D-pad left/right | Left/right | Seek one second while paused |
| X | R | Restart the same deterministic round |
| Y | M | Switch River Valley / Classic Tanks |
| LB | S | Toggle quarter speed |
| B | D | Toggle goal lines |
| LT | V | Toggle combined team sight |
| — | Esc | Exit spectator |

While paused, the mouse/touchpad can scrub the timeline. Clicking the map marks
coordinates to refer to in corrections. The four-minute round stays open at its
end for review. Observer snapshots use compact geometry deltas and five-second
seek checkpoints; review never modifies the live simulation. Replay frames show
recorded geometry and turret poses; transient sparks are shown only live.
Recordings live under `~/.local/share/fracture-ai-arena/`, with metadata,
compressed snapshot packets, input/event logs, compressed team sight and a summary. F12 captures the
native renderer directly, including on Wayland desktops where X11 screen grabs
can be black.

## Headless development harness

`tools/ai-arena.py` runs silent, recorded headless matches using the actual game
physics, weapons, damage, respawns, defensive guns and flag rules. Decisions run
at 10 Hz per bot; physics runs at 120 Hz. A round simulates **28,800 ticks / four
minutes** by default. A capture scores a team point and starts a fresh leg within that round. Two workers
per team share a frozen version, with separate controller state and random
streams. Human multiplayer placement, shared sight and respawn rules apply.

## Build and start

```sh
FRACTURE_SILENT=1 cargo build --release --locked -p sim --example ai_arena --features physics-profile
python3 tools/ai-arena.py freeze target/ai-arena/versions/v13 tools/ai/versions/v13.json
python3 tools/ai-arena.py freeze target/ai-arena/versions/v11 tools/ai/versions/v11.json
python3 tools/ai-arena.py run --blue target/ai-arena/versions/v13 --red target/ai-arena/versions/v11 --output target/ai-arena/round-new-01
```

The default map is River Valley. `--seconds 240` sets duration and `--seed 42`
sets the repeatable random stream. Each bot chooses uniformly among scout,
tank and mortar at initial deployment and after every death; it keeps its choice
through the ten-second respawn delay. Initial deployment is immediate. The native viewer seeds new launches from the clock;
`--seed` reproduces one, and R replays the same seeded round. Scout controls use
rolling steering; mortar shots align the hull and respect the 6–36 m range.

Select another built-in or saved map with
`--map levels/classic-tanks-2.json`. Each output directory must be new. No game
window, audio device, saved map or player preferences are used.

Open the round's `replay.html` in a current browser. It is self-contained, silent,
seekable and supports playback speed plus observer/Blue/Red visibility views.
Visual coordinates are rounded to a millimeter to reduce file size. The gzip
decision and frame logs retain original precision. To regenerate the viewer
after a viewer change:

```sh
python3 tools/ai-arena.py replay target/ai-arena/round-01
```

## Improve the loser, retain the winner

Inspect `summary.json`, `replay.html`, `decisions.jsonl.gz` and `events.json.gz`.
The summary includes stationary stalls (including failed clearing), their times
and locations tagged by player, movement, recovery attempts, shots, team kills, objective
progress, decision time, visibility time, physics tick timing and slow ticks.
Slow ticks include a stage breakdown. Setup costs are separate in new captures.
Nested structural/rebuild timers must not be added to their parent stage.

The league selects a winner by captures, enemy player deaths, damage received by
the opposing player, then closest approach to the enemy flag. These are
lexicographic tiebreakers to select the next opponent; normal game time-limit
draw rules are unchanged. A complete tie retains the incumbent. Team kills can
come from defensive guns; the metric does not claim every kill was a bot shot.
Damage is an observer-side approximation sampled at decision times. In 2v2,
team movement and stall seconds sum both vehicles; stall time can exceed the
round duration. `player_stall_seconds` and `stuck_actors` identify individuals.
Historical 1v1 and new 2v2 results are separate comparison groups.

Change the losing controller or its configuration based on that round's
evidence, build again and freeze it under a new version directory. For example,
after v3 beats v1 in round two:

```sh
python3 tools/ai-arena.py freeze target/ai-arena/versions/v4 tools/ai/versions/v4.json --parent target/ai-arena/versions/v1
python3 tools/ai-arena.py run --blue target/ai-arena/versions/v3 --red target/ai-arena/versions/v4 --previous target/ai-arena/round-02/summary.json --output target/ai-arena/round-03
```

With `--previous`, Blue must match the previous winner's complete manifest and
Red must name the previous loser as its parent. Each manifest pins executable,
configuration and simulation source archive hashes. Never rebuild a winner in
place: its executable runs in a separate worker process, so shared-source edits
cannot change it. The first two versions differ in settings; later versions
also have different frozen executables. The archive records simulation sources
and the Cargo lockfile for review; building it still requires the rest of this
workspace and its embedded map assets.

Keep an incumbent frozen until its challenger demonstrates objective dominance.
The mirrored-series harness plays both colors for each seed and requires at least
four complete rounds: at least 75% flag-score wins, no flag-score losses, and at
least three times the opponent's total captures. Kills, damage and distance do
not turn a flag draw into a promotion. For example:

```sh
python3 tools/ai-league.py --incumbent target/ai-arena/versions/v11 --challenger target/ai-arena/versions/v16 --output target/ai-arena/series-v16
```

`series.json` pins both versions and all round summaries. Only a passing series
switches development to the old incumbent's branch. `--summary-only` on the
single-round harness keeps metrics and compact `evaluation.jsonl` telemetry for
quick iteration; it does not produce a visual replay. Use a full recorded round
and the native Deck spectator to review the final candidate. An immutable copy
of the host harness can be passed with `--binary` while compiling new candidates.

This is a development loop: review recordings and implement the next losing
version, then invoke the next round. The harness enforces version identity and
parentage; it does not invent algorithm improvements. Use several maps and
mirrored sides before drawing broad conclusions about strength. A single
round result can be affected by map asymmetry and defensive guns.

## Breach and team regression probes

`ai_siege` isolates actual River Valley base walls by placing one selected vehicle
twelve meters from the opposing flag. It preserves the map's wall materials and
physics. This is a regression scenario, not a competitive league round:

```sh
FRACTURE_SILENT=1 cargo build --release --locked -p sim --example ai_siege
FRACTURE_SILENT=1 target/release/examples/ai_siege target/ai-arena/versions/v21/agent target/ai-arena/versions/v21/config.json blue tank 0 /tmp/base-probe.jsonl --no-defences
```

Directions 0–3 approach from east, south, west and north. Repeat for both teams
and tank/mortar. Omitting `--no-defences` retains defensive guns. The trace stores
positions, observed wall outlines, reticle spread, controls and goals; its summary
records capture time, shots and muzzle obstructions. Forced vehicle/spawn choices
are exclusive to this probe. League and Deck rounds retain independent uniform
random choices, including the possibility of two scouts. Scouts cannot shoot
through base walls and must use existing openings or support a breaching ally.

`ai_navigation` isolates driving on River Valley, with stationary undeployed
opponents and no fixed guns. It drives each chassis across the central river
crossing and around the large rocks in both directions, using normal physics,
delayed observations and the frozen worker. Combat and base-assault decisions
are disabled for this fixture. Public waypoints have room for the largest hull;
traces retain controls, heading, health, progress and recovery modes.

```sh
FRACTURE_SILENT=1 cargo build --release --locked -p sim --example ai_navigation
FRACTURE_SILENT=1 target/release/examples/ai_navigation target/ai-arena/versions/v40/agent target/ai-arena/versions/v40/config.json target/ai-arena/navigation-new
```

Safe navigation treats scenery as blocked during travel, preserves the same hull
clearance in planning and lookahead, and holds a reachable path endpoint when a
goal is enclosed. It never substitutes a straight drive through an unreachable
goal's obstacles. Scouts evaluate physical rolling/reversing arcs. Tracked hulls
turn before advancing into tight bends. Recovery and teammate yielding also
check clearance; deliberate base-wall breaching remains a separate behavior.

## Information boundary

`sim::ai::Controller::decide` accepts an `Observation` and returns the same
`PlayerInput` as a human multiplayer client. The worker branch reads only its
initialization and observations from stdin. It receives no `World`, physics
queries, live level recipe, global statistics, hidden object IDs, enemy health,
reloads, respawn timers, velocities or unseen enemy positions.

The observation adapter applies the human `player_vision` rules: vehicle cone
or scout circle, range, cover, grass concealment, firing reveals, radar and
allied sight. It sends own vehicle state, currently visible enemy figures,
visible obstacle outlines, visible terrain samples, visible available pickups,
visible allied vehicle types/positions, own special inventory and public
bounds/flag markers. Both bots also receive
an immutable starting navigation grid from the authored map shown in the human
lobby preview. It contains terrain, intact bridges and scenery, with no mobile
vehicles or hidden live state. Authored fixed guns have a separate immutable
list of starting positions, ownership, facing and traverse arcs. Controllers can
route around those initial firing sectors, but receive no hidden health, current
aim, capture or destruction updates. A gun's route penalty is removed only after
delayed visible evidence that it is gone or allied. Unseen destruction never
updates the starting grid. Whole-figure outlines follow the human fog renderer's visibility
semantics. The controller remembers observed terrain/cover and last sightings;
unseen movement or destruction never refreshes that memory. Dead-player sight
does not disclose hidden enemies. Observer recordings are collected separately
and are never sent to either worker.

All new controllers delay sensor information by at least **300 ms / 36 physics
ticks** before acting on it. Decisions run at 10 Hz, with no instantaneous
reaction to newly seen targets, cover or pickups. Acquiring a special also has
a 300 ms activation delay. Own motion and inventory stay current. Turret
traverse, main-gun dispersion, reload and vehicle physics remain authoritative;
the controller never reads hidden enemies or compensates the random shot offset.
It may estimate movement from consecutive delayed sightings and wait for its own
reticle to settle. Public team slots and delayed allied sightings can determine
breacher/support roles; roles never alter independent random vehicle selection.

New team controllers assign a tank to breach, or a mortar when no tank is
available. A public team-slot tie chooses between equal vehicles. Tank/mortar
support stays behind and to the side; scout support screens ahead. Support covers
the breacher and yields in tight passages. Last observed teammates are remembered
when they leave sight; stale positions are never refreshed from hidden state.
After losing contact, support searches onward rather than waiting forever. Two
scouts take flank/guard roles and use existing openings. The native sidebar names
these roles. Decisions remain separate worker processes; cooperation uses delayed
friendly sightings and public roster slots, with no hidden enemy state.

Bots detour to nearby visible pickups when their vehicle supports them. Rocket,
laser and plasma activation checks aim, range, cooldown and visible allies in
the firing lane. Mines are placed near objectives or observed threats and spaced
at least eight meters apart. Scouts retain the player's restriction against
carrying/using specials. No weapon is granted directly by the harness.

The local game opponent uses this same boundary and human control path. Hunting
uses recent sightings and searches public objectives when it has no sighting;
it no longer refreshes the player's coordinates through walls. Ammo switching,
turret traverse, dispersion, reloading, movement and destruction use the human
routines. Driving-only editor opponents retain their non-firing patrol mode.

Regression checks compare observations/actions when only hidden enemy state or
hidden cover changes, test cover/cone/grass detection for both sides, and compare
the bounded observation scan against a full scan for both teams and all vehicle
sight profiles. This is a reviewed built-in controller interface, not an OS
sandbox for hostile third-party executables.

## Performance interpretation

The profiler measures headless simulation and controller CPU time, not rendered
frame rate or Steam Deck FPS. JSON recording and replay generation are outside
physics tick timings. Bot round-trip timing includes serialization, IPC and the
controller's own CPU time; bot CPU is reported separately. Both players use the
same decision cadence, inputs and physics. A worker has five seconds to reply;
missing/malformed/invalid replies fail the run and terminate workers.

The first pass found a full-map terrain scan and unnecessary polygon visibility
tests for distant objects. Sampling now visits the union of sight-region bounds
and rejects distant obstacle bounds before exact visibility tests, retaining
the same observation contents. The rounds also exposed too-close clearing,
aiming beside the wall instead of at its surface, clearing cover beyond a path
bend, repeated short reverses, and backing toward blocked ground. Versioned
changes address these with firing space, actual corridor checks, visible fire
lanes, a ten-second progress history, temporary route avoidance and observed
retreat clearance. Public-layout planning now distinguishes impassable terrain,
solid props and expensive breakable routes; clearance costs favor open space
without sealing small flag approaches. Lookahead checks the swept corridor so
it cannot shortcut a river bend or cut through a wall. Physical collisions remain authoritative.

Recordings and frozen workers live under ignored `target/ai-arena/`; compact
checked-in results live under `docs/validation/`. See the dated validation report
for measured results and remaining failures.

The v6/v7 exhibition upgrades both v4/v5 branches for the newly requested map,
reaction, vehicle and special rules. Version v8 adds a bounded clearing timeout and broader detours after the four-minute
recording exposed scouts repeatedly firing into the same base wall. It runs
against the unchanged v6 winner. Version v9 retains that lineage and makes
scouts treat breakable cover as blocked, patrol around contested objectives,
flank visible enemies and fire only at vehicles. Tanks retain sustained
cover-clearing behavior. Earlier frozen versions and 120-second
recordings are retained unchanged. Subsequent league rounds still retain the
previous winner and improve the loser.


The v10/v11 2v2 exhibition applies the user's spacing and ineffective-fire rules
to both earlier branches. A bot backs away from enemies inside its usable firing
range, considers cover and visible allies before retreating, then holds a clear
firing lane. Scouts steer while reversing rather than requesting a stationary
pivot. A clear, nearby capture remains a priority over standing off indefinitely.
Wall attacks compare visible occupancy near the impact and the bot's own reload
transitions. After at least six shots and twelve seconds without visible progress,
the bot excludes that approach temporarily and tries a flank. New visible breach
progress resets the timer. No material health or hidden destruction is queried.

Respawn cooldown is now **10 seconds** after destruction in both normal matches
and the arena. The host owns the timer; choosing a vehicle early cannot shorten
it. Native recordings include `respawn_seconds`, and each vehicle row displays
its remaining cooldown. Earlier recordings retain their historical three-second
rule and must not be compared as if the match rules were identical.

The first Deck 2v2 round ended 6–7 in team kills, so v11 was retained as the Deck
winner and v13 improves the losing v10 branch. V13 raises its combat hold range
from 6 m to 12 m to avoid pushing back into point-blank fights immediately after
opening range. The current package defaults are Blue=v40, the navigation/team
challenger, and Red=v11, the frozen incumbent. This is a development pairing,
not an objective-dominance promotion. Native and headless outcomes differed; cross-device bit identity
and strength gains from a single round are not claimed. The new ten-second
respawn rule also prevents a controlled score comparison with the prior round.

Measured results and remaining failures are in the
[2026-10-18 validation report](AI_ARENA_VALIDATION_2026-10-18.md).
