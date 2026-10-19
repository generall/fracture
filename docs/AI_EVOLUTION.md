# Five-iteration AI evolution

Active user goal: at least five iterations with clear domination; improve the losing branch against the frozen winner, then switch branches. Navigation-only improvements and tied matches do not count.

The persistent ledger is `docs/validation/ai-evolution-state.json`. It currently has **0 of 5** verified promotions. The incumbent is frozen v11; the challenger branch continues from v40.

A promotion requires four complete four-minute 2v2 matches, both colors for each of at least two seeds, at least 75% flag wins, no flag losses and at least 3:1 captures. Ten-second respawns and independent uniform vehicle picks are unchanged. Matches must run natively on the Deck. Recorded FlagCaptured physics events must agree with summaries. Executables, configuration, source, map and harness hashes are pinned. The next challenger must descend from the previous losing branch; the winner remains unchanged.

`tools/ai-league.py` evaluates host development trials. `tools/ai-deck-series.py` stages frozen versions and runs resumable silent matches in owned Deck user services, with two tests at a time. A disconnected SSH session does not terminate those services. `tools/ai-evolution.py promote --series PATH` validates and records a qualifying Deck series, then changes the branch to improve. It preserves each promotion proof as a separate compressed artifact.

## Current experiments

**Architecture reset:** v41–79 did not establish a promotion. Parameter sweeps
are stopped. V80 replaces the movement override stack with a persistent mission,
one selected movement task, complete-route caching and separate fire control.
See [the redesign and behaviour contracts](AI_CONTROL_REDESIGN.md). The river,
passing, both-base breach and guarded spawn-to-capture contracts pass. Native
competitive dominance remains unproven; the ledger is still 0/5.
V80 failed the native mortar garage exit (18/19 navigation cases) and a repeated
team contract, so it was rejected. V81 corrects the handoff between global
routing and local driving; its task contracts and complete guarded-base team
contract pass. All 19 native Deck driving probes pass with 100 HP. One native competitive
smoke finished 0–0 on flags, 7–7 enemy deaths. No wall-clearing task was reached
under live combat. The next work is complete mission/traffic behaviour and
combat arrival, not parameter searches. It is not a promotion series.

- V41: global base approach and raiding past mobile fights. Four host matches drew on flags.
- V42: committed roles for each life and tighter breach steering. Four host matches drew on flags.
- V43: visit public pickup locations to arm for a breach; hidden pickup type/availability remains unknown. Four host matches drew on flags.
- V44: wider routes, reserve offensive specials for base walls, clear visible fixed guns when necessary. Four host matches drew on flags. The Deck viewer and mirrored Deck test services use this candidate for observation.
- V45 is an unpromoted intermediate snapshot. A review found its ruin visibility needed the same foliage override as the human renderer; V46 fixes that before competitive evaluation.
- V46: faster scout screening and human-visible ruin memory, including 300 ms delay. Four host matches drew on flags. A rocket breach probe showed premature entry left remaining rockets unusable inside the safety range.
- V47: rocket standoff and visible-clearance breach entry. Four host matches drew on flags.
- V48/V49: route-aware breaches and more precise approach to flag posts. V48 was corrected before competitive evaluation; four V49 host matches drew on flags.
- V50: lower route cost through a committed breach and physical entry after a burst. Both tank rocket probes captured from opposite colors, but all four full matches drew on flags.
- V51: retrace the pickup entrance when the coarse travel grid seals a narrow exit; deterministic traffic priority. Four host flag draws.
- V52: prefer arming at a closer public site and finish a visibly clear flag approach. Four host flag draws.
- V53: only the breacher makes supply detours; route around delayed visible allies and back out farther before yielding. A real head-on tank test failed before these changes and passed afterward. Four host flag draws. The silent 19-case navigation suite passed natively on the Deck, all cases retaining 100 HP.
- V54: align the base approach more precisely and follow the actual breach line inside the base; optional public supply runs. Unarmed tank probes captured on both colors in 40–42 seconds. Four host matches produced one challenger flag win and three draws, which does not qualify for promotion.
- V55: full-hull clearance at the staging point, lane-aware asymmetric traffic, and enough turning input for a yielding mortar. Real head-on tank/mortar pairings and the River Valley escort case pass. One host flag win, three draws.
- V56: consistent waypoint stopping/arrival radii and reverse arrival instead of a 180-degree turn for a close tracked goal. All 19 driving cases pass natively on the Deck with **raiding navigation enabled**, all retaining 100 HP. One host flag win, three draws. Installed in the Deck viewer.
- V57–V59: shortest threat-free travel, independent assault and wide flanking alternatives. Each four-match host series drew on flags.
- V60: damaged breachers return fire at a visible close mobile threat and resume clearing afterward. Four host flag draws.
- V61–V64: proactive interception and alternative engagement ranges. V62 won one host match on flags; all other matches in this group drew.
- V65: recognize a normally visible enemy chassis through the same 300 ms sight delay, prioritize mortars for scouts, and keep tanks moving while engaging a mortar. Hidden and shot-only chassis remain absent. All four native Deck trials drew on flags; 40:16 deaths did not qualify it for promotion.
- V66–79: approach reachability, steering damping, mobile combat, persistent fronts and perimeter routes. Some host trials produced one flag win, but none met the promotion gate. V67 passed the 19-case native navigation probe; crossing times improved in some directions and worsened in others. The Deck viewer subsequently moved to the new task controller in v81.
- V80–81: replace movement overrides with a persistent task executive. V81 passes 19/19 native navigation cases, all with 100 HP. Its one native four-minute smoke match drew on flags; no promotion.
- V82–83: independent scout missions, visible-motion fire control and planned aim samples at the physics input rate. Each four-match host series drew all flag games. V83 killed a crossing scout in the isolated tank probe where V82 never fired; this remains diagnostic evidence.
- V84: complete perimeter approach and guarded-base integration contract captures at 177.6s Blue / 210.8s Red. Fixed tank/scout choices and stationary mobile opponents make this a behavior check, not a promotion. Its four host comparisons all drew on flags (22:16 kills). No promotion.

Current source also has an unfrozen scout motion-model correction. Its garage
regression passes before the old 20-second passing timeout; all 19 host driving
probes retain 100 HP, and its guarded team contract captures at 183.6s Blue /
174.0s Red. It has not been evaluated as a competitive candidate. The user then
requested a recorded human 1v2 against frozen v11; that mode now runs in the
separate Deck application. Do not restart it for AI tests during the user's play.

All native tests use FRACTURE_SILENT=1. Workers still receive only public starting-map data and delayed human-visible observations. New public pickup markers contain positions only, as shown in the lobby preview; hidden availability, weapon contents and cooldowns are not provided. Visible ruined guns follow the exact human fog/foliage checks and the same sensor delay.

Latest detailed traces and summaries are under `target/ai-arena/series-v41` through `series-v53`, plus focused `siege-v46-*-rocket.jsonl` probes. Deck services are named `fracture-ai-lab-*`; the separate battlefield viewer remains `fracture-ai-arena.service`. The lab does not modify the installed normal game or player preferences.
