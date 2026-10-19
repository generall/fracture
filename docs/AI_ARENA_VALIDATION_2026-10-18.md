# AI arena validation — 2026-10-18

The rounds below are historical. Current navigation and team coordination
results are in the [navigation follow-up](AI_ARENA_VALIDATION_NAVIGATION_2026-10-18.md).

The separate native spectator runs four-minute **2v2** matches on the Steam Deck.
B1/B2 and R1/R2 have independent controllers and random vehicle choices, shared
human team sight, special pickups and the same ten-second respawn cooldown as
the normal game. Blue is v13, the improved losing Deck branch; Red remains the
frozen v11 Deck winner. Both use a 300 ms sensor/activation reaction delay.

The final native v13/v11 round also completed 28,800 ticks with the ten-second
cooldown, ending 7–7 team kills and no flag captures. Pause/seek was checked
again, then a fresh round was left live for review.

The full field has current Blue/Red vision masks, goals and stall markers.
A/Space pauses, D-pad/arrow keys seek, X/R replays the seeded round, Y/M changes
maps, and LT/V toggles team sight. The desktop launcher is **Fracture AI Battlefield**.
Recordings are under `~/.local/share/fracture-ai-arena/` on the Deck.

## Recorded rounds

[Open the recording index](../target/ai-arena/index.html), or [the latest headless replay](../target/ai-arena/round-09/replay.html).
The replay links refer to the host workspace; native Deck recordings are stored separately.
[Machine-readable results](validation/ai-arena-2026-10-18.json.gz) preserve full
summaries, hashes, timing stages, stall locations and the calibration evidence.
Frozen executables, configurations and simulation source archives remain in
`target/ai-arena/versions/`; they are never overwritten.

| Round | Teams | Duration | Map | Blue / Red | Winner | Team kills B/R | Stationary stall seconds B/R |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | 1v1 | 120 s | River Valley | v1 / v2 | v1 | 1 / 0 | 117.0 / 12.8 |
| 02 | 1v1 | 120 s | River Valley | v1 / v3 | v3 | 0 / 0 | 117.0 / 30.2 |
| 03 | 1v1 | 120 s | River Valley | v3 / v4 | v4 | 0 / 1 | 47.7 / 30.4 |
| 04 | 1v1 | 120 s | Classic | v4 / v5 | v4 | 4 / 3 | 37.6 / 30.4 |
| 05 | 1v1 | 240 s | River Valley | v6 / v7 | v6 | 2 / 0 | 99.3 / 92.7 |
| 06 | 1v1 | 240 s | River Valley | v6 / v8 | v6 | 2 / 0 | 91.7 / 37.8 |
| 07 | 1v1 | 240 s | River Valley | v6 / v9 | v9 | 2 / 4 | 32.0 / 49.3 |
| 08 | 2v2 | 240 s | River Valley | v10 / v11 | v10 | 9 / 6 | 37.8 / 48.8 |
| 09 | 2v2 | 240 s | River Valley | v11 / v13 | v13 | 5 / 7 | 47.2 / 69.6 |

Stall detection includes failed clearing and stationary turns, excludes deliberate
combat holds, and requires three seconds within 0.75 m. For 2v2, time sums the two
vehicles per team; use `player_stall_seconds` and `stuck_actors` for individuals.
Kills include allied defensive guns. Winner selection is captures, team kills,
observed damage, then closest approach to the opposing flag. No recorded round
captured a flag; objective completion and some navigation stalls still need work.

Rounds 1–4 used 120 seconds. Rounds 5–7 used 240 seconds with starting map knowledge,
reaction delay, random vehicles and specials. Rounds 6 and 7 retained v6 while
improving the losing v7/v8 branches. V9 won the final 1v1 development round.
The 2v2 request then upgraded both branches to v10/v11. Round 8 used the old
three-second cooldown. Its headless result was 9–6, while the actual Deck round
ended **6–7**, with 28,800 ticks and 2,401 snapshots. Native and headless outcomes
are reported separately; cross-device bit identity is not assumed.

The Deck winner v11 stayed frozen. V13 improves the losing v10 profile by holding
firing range at 12 m instead of pressing into a 6 m fight. Round 9 validates this
pair under the newly requested **ten-second cooldown**, with the incumbent moved
to Blue by the headless league convention. The Deck retains Blue=v13, Red=v11.
Side assignment and cooldown changed, so these scores do not establish a
controlled strength improvement. Several maps and mirrored sides remain needed.

## Failures addressed

- Scout guns cannot damage scenery. Scouts now route around breakable walls,
  patrol around blocked objectives and flank visible opponents instead of firing
  indefinitely into a base wall.
- Other wall attacks compare only visible cover near the impact and the bot's own
  reload transitions. At least six shots and twelve seconds without visible
  progress trigger a temporary exclusion and flank. A visible breach resets the
  timer; productive tank clearing remains covered by a regression test.
- Vehicles inside usable firing range back away along a clear mapped direction,
  accounting for cover and visible teammates. Scouts reverse with car steering.
  A clear firing lane becomes a combat hold; a nearby clear capture still takes
  priority. A physical two-tank test proves separation and resumed firing.
- Bots move out of a teammate's firing lane and suppress shots through that ally.
- Navigation begins with the immutable authored layout shown in the human lobby,
  including intact crossings. Clearance costs, corner checks and temporary
  exclusions improve routing; unseen live changes never update that layout.

The controller receives own state, visible enemies, visible cover, visible
pickups and the public starting layout. It receives no hidden enemy positions,
health, reloads, velocities or material health. Sensor updates wait at least
36 physics ticks; both teams decide at 10 Hz through normal human inputs.
Turret speed, dispersion, gun ranges, inventory restrictions and cooldowns are
resolved by the actual game. Tests exercise collection/activation of rocket,
laser, plasma and mines, plus hidden enemy/cover/pickup changes and vehicle sight.

## Performance and recording

The controlled visibility calibration reduced mean observation work from
**6.803 ms to 1.933 ms (71.6% less)**;
P95 fell from 7.375 to 2.270 ms. All
2,400 original observations and input packets were
identical after the bounded-scan optimization, with matching SHA-256 digests.
This calibration predates the later AI interface changes and measures headless
CPU work, not rendered Deck FPS.

The latest 2v2 headless run measured physics mean 0.357 ms,
P95 0.679 ms; observation mean 2.018 ms;
controller mean 0.345 ms per decision. Structural fracture
remains the source of occasional long ticks (round 8 reached 21.5 ms). Nested
fracture/rebuild timers must not be added to their parent stage.

Team masks render each source union once per 10 Hz update. Native recordings
store sight as compressed millimeter coordinates instead of full JSON ray fans;
seek checkpoints occur every five seconds. The first complete 2v2 Deck recording
used 19.7 MiB of geometry packets,
41.7 MiB of sight and
3.2 MiB of input/event logs.

Deck display sync initially throttled the new viewer to about one frame per
second. The launcher now sets `vblank_mode=0` only for its Deck child process and
holds temporary idle/display inhibitors. Live samples reached 152–190 FPS after
the cooldown update; these are samples, not sustained frame-rate guarantees.
No saved display, mixer, map or player settings were changed. Internal tests used
`FRACTURE_SILENT=1`; the isolated replay browser used `--mute-audio`.

## Verification

- 376 workspace tests passed; two optional unit tests and four socket suites stay
  ignored by default. The targeted multiplayer plasma/death/respawn socket test
  was run explicitly and passed with the ten-second cooldown.
- Exact-tick tests keep destroyed vehicles dead through tick 1,199 and respawn on
  tick 1,200 when a choice and clear spawn are available. Initial deployment stays
  immediate, and deployment remains independent for all four players.
- Strict Clippy passed with the existing three repository lint allowances.
- The normal game's WASM target check and both native release builds passed.
- Native pause/seek restored recorded vision; compressed sight and seek have a
  round-trip test. Browser recordings passed observer/Blue/Red view checks,
  seeking, playback and the exact 240-second endpoint without script exceptions.
- The normal Deck game binary was also updated to the shared ten-second rule;
  it was not launched over the separate spectator.
