# AI navigation and team validation — 2026-10-18

The development pairing is Blue=v40 (`clear-navigation-and-reverse-recovery`)
against frozen Red=v11. Basic navigation took priority after the newer Deck
version repeatedly contacted rocks and river banks. No objective-dominance
promotion or switch to improving v11 has occurred.

## Driving evidence

The `ai_navigation` example uses actual River Valley geometry and physics,
frozen worker executables and delayed human-visible observations. Combat and
base assault are disabled, with no fixed guns or deployed opponent. It forces
each chassis for the fixture; competitive matches retain independent uniform
vehicle choices. These probes are not competitive results.

The previous Deck build, v32, passed **8/12** original routes. Both tracked
chassis failed both river directions within 90 seconds. Five runs lost health
through contact. V40 passed **19/19**, each with **100 HP retained**:

- Scout, tank and mortar cross the central river in both directions.
- All three pass the large rocks in both directions.
- All three leave the actual Blue spawn and return to its open approach.
- The scout also enters the garage, testing slow arrival in a confined space.

These tests verify arrival within two meters, track progress and stationary
intervals, and retain position, heading, input and controller telemetry. Turning
and recovery still take time; zero damage does not imply zero physical contact.

## Changes

Travel routes treat scenery as blocked instead of choosing to ram or shoot
through rocks. Planning and lookahead preserve the same full-hull clearance.
An unreachable goal retains its reachable endpoint rather than triggering a
straight drive through obstacles. A source inside the conservative clearance
margin can plan an escape without crossing actual blocked cells.

Tracked vehicles turn before applying forward force into tight bends, with
enough input to overcome ordinary friction. Scouts evaluate rolling and reverse
arcs, with slow inputs near their destination. A stuck scout's recovery selects
from reverse arcs, preventing endless tiny forward commands. Recovery and team
yielding check remembered clearance. Vehicle physics and weapon balance are
unchanged by this navigation update.

Separate team controllers choose complementary breach/support or flank/guard
roles. Friendly whole-figure visibility matches the human renderer. Roles
remember delayed friendly sightings; support searches onward after losing
contact and yields through checked paths. No hidden friendly or enemy state is
refreshed. The actual two-tank spawn regression verifies progress and separation.

Public starting fixed-gun positions and authored firing sectors guide routing;
their hidden live health, aim and ownership remain unavailable. A gun's routing
penalty changes only after visible evidence. Its initial costs are cached.
Observation geometry is also built once per four-player batch, then filtered
separately. A 400-observation benchmark preserved complete observations and
reduced mean batch cost from 8.447 ms to 6.488 ms on the host; this is not a Deck
frame-rate claim.

## Competitive limits and checks

Both V38 and V40 completed four mirrored rounds each, seeds 42 and 137 on both
colors, all without captures. The prior actual Deck v32/v11 round also ended 0–0 captures
and 6–6 kills. Navigation regression success does not establish objective
dominance; base assault still needs improvement. V11 remains frozen and the
promotion gate uses flag results rather than kills to break draws.

The final source passed **394 workspace tests**, with six ignored, strict
Clippy (the repository's existing lint allowances), WASM compilation, formatting
and diff checks. Native tests and the Deck viewer use `FRACTURE_SILENT=1` without
changing saved mixer settings.

[Machine-readable evidence](validation/ai-arena-navigation-2026-10-18.json.gz)
pins frozen versions, map and probe hashes, all driving results, the mirrored
series, the prior Deck result and the observation benchmark. Detailed traces
remain under `target/ai-arena/navigation-v40/`. See [AI arena](AI_ARENA.md) for
commands and Deck recording controls.

The deployed v40/v11 viewer was checked on the Deck at 1280×800 with all four
actors, live team masks and role labels. At 30 seconds, the Blue tank had crossed
the central river at full health; at 40 seconds, the scout had passed the rock
corridor and reached the central crossing at full health. The native screenshot
is `target/screenshots/ai-navigation-v40-deck.png`. Live FPS samples varied with
combat; no locked frame-rate guarantee is claimed.

The actual Deck v40/v11 round completed all **28,800 ticks / 240 seconds**,
ending **0–0 captures and 11–10 kills**. This is an objective draw. The complete
native recording is retained on the Deck as `round-1792347399866` and copied to
`target/ai-arena/deck-v40-recording/`, including frames, vision, input/telemetry,
metadata, summary and screenshot.

## Follow-up evidence: V53–V55

V53 repeated all 19 isolated driving checks **on the Steam Deck**, with every
case reaching its destination and retaining 100 HP. The exact native executable
hash, frozen agent identity and all outcomes are recorded in
`validation/ai-navigation-deck-v53-2026-10-18.json.gz`. Combat and team coordination
are disabled in these isolated probes; they do not prove a whole match is free
of stalls.

Full-match traces exposed additional coordination and staging errors: support
vehicles competed for the breacher's supply, both vehicles could reroute around
one another indefinitely, lane yielding stopped vehicles that could pass, and
the southern Red-base staging point lay inside a small rock enclosure. V55
reserves supply detours for a breacher, makes the priority vehicle retain its
route, leaves passing room for the other vehicle, and requires clearance for
the complete hull at a base staging point. Dedicated physics tests cover all
four tank/mortar head-on pairings and an actual River Valley tank/scout escort.

V54's aligned breach captured with an **unarmed tank** from both colors in
40–42 seconds in focused fixtures without defenders. Its competitive host
series won one match on flags and drew three; it is **not** a dominance
promotion. The five-iteration evolution goal remains active with no verified
promotions recorded yet.

V56 repeats all 19 cases with **raiding navigation enabled**. It fixes a mismatch
between the 40 cm steering stop and 25 cm route-advance threshold, retains the
usual tolerance for intermediate cells and uses a smaller final arrival radius.
A tracked vehicle can reverse to a nearby staging point instead of making two
180-degree turns. Every native Deck case reached its goal with 100 HP; proof:
`validation/ai-navigation-deck-v56-2026-10-18.json.gz`. This is the currently
installed Blue AI in the separate Deck viewer. The viewer explicitly limits its
Deck render rate because the compositor requires the no-vsync workaround.
