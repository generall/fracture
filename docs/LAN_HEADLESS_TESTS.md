# LAN headless validation — 2026-09-26

The user requested headless testing before further desktop checks. The two test
game windows were closed. `tools/check-lan-headless.sh` explicitly unsets both
`DISPLAY` and `WAYLAND_DISPLAY`; it does not launch Macroquad, a browser or audio.

## Coverage

Simulation tests cover six independent tanks and reloads, remote specials,
shared pickup timers, invalid controls, disconnect forfeits, human respawns,
empty client physics worlds, network interpolation endpoints, and shared-visibility
cache equivalence/invalidation.

Protocol tests cover bounded compression and fragmented frame reads, malformed
headers, binary map transfer, persistent geometry, geometry changes without motion,
visibility exclusion and ordinary visible enemies, disappearance/re-entry, action
counter wraparound, duplicate/reordered/invalid input, and stale-input braking
without losing accepted one-shot actions.

Four socket tests use actual host threads and real loopback TCP/UDP:

1. Host plus five guests: LAN discovery, readiness gate, independent movement,
   stale-input braking, leaving and lobby reset/map revision handling.
2. Time-limit result at exactly 120 ticks, return to lobby, new map transfer and
   a second match ending with flag capture, consistently on every client.
3. A remote player's single-use plasma pickup, one-second charge, duplicate-input
   rejection, crate destruction, destruction of the host tank and its respawn.
4. A deliberately tiny TCP receive window and a client that stops reading while
   continuing to send input. Other clients progress; snapshots are skipped for the
   slow reader, and its existing delta decoder recovers after reads resume.

The fourth test actually reaches socket backpressure. Earlier runs skipped 28
snapshots with a peak application queue of 29,861 bytes; it is not merely an idle
client test.

## Findings fixed during testing

- Ordinary visible tanks must fall back to polygon visibility when there is no
  special tall-grass visibility override.
- Filtered pickup/bridge arrays need stable position-based identities in client fog.
- Snapshot work must be included in the 50 ms sending period, not added to it.
- Rebuilding blocker classification and painted grass for every player's sight
  query dominated snapshot work. A cache shared across the batch preserves exact
  results and reduces that cost.
- Unread peers need a bounded kernel send buffer as well as an application limit.
- Accepted one-shot actions must survive a host scheduling stall while stale
  movement brakes. Dedicated deterministic tests cover that distinction.
- Per-player shot totals are carried independently of hidden global diagnostics.

An initial concurrent socket run timed out in the plasma scenario. It passed in
isolation and in three consecutive complete runs. The subsequent one-shot input
change adds deterministic stall coverage; the final script result below is the
current acceptance evidence, rather than treating that initial timeout as proof
of success.

## Current result

`bash tools/check-lan-headless.sh` completed with **exit status 0**, with both
`DISPLAY` and `WAYLAND_DISPLAY` unset throughout:

- **250 workspace unit tests passed:** 204 simulation, 36 client, 10 network.
  The existing manual simulation timing test remains ignored.
- All **four real-socket integration scenarios passed** when explicitly enabled.
- Formatting passed. Clippy passed with warnings denied and the three pre-existing
  project allowances (`chunks_exact_to_as_chunks`, `items_after_test_module`,
  `approx_constant`).

Release benchmark on Linux x86-64, AMD Ryzen Threadripper PRO 7965WX: one host
thread and six actual localhost TCP clients, ten seconds of movement and firing
following the countdown. River Valley is 160 × 160 m with 116 authored objects;
the run reached **5,139 active authoritative bodies**.

| Measurement | Median | p95 | p99 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Host physics tick | 1.350 ms | 2.319 ms | 4.181 ms | 13.056 ms |
| All six snapshots | 4.916 ms | 5.533 ms | 5.962 ms | 6.708 ms |
| Client snapshot interval | 50.311 ms | 51.745 ms | 52.605 ms | 52.731 ms |

The host simulated 1,202 ticks with **zero catch-up caps**. Each client received
260 snapshots including countdown updates. Six snapshot opportunities were
skipped by backpressure pacing during the run; reliable delta chains remained
valid. Received gameplay traffic was 106–286 KiB/s per client (about 1.05 MiB/s
combined). Peak visible-body counts per client were 450, 56, 196, 10, 228 and 66,
well below the authoritative body count.

The deliberately slow-reader test skipped 28 updates with a peak application
queue of 29,861 bytes, while the other clients continued. Resuming reads recovered
without resetting the decoder or losing the session.

Raw command output is retained in
[`validation/lan-headless-2026-09-26.log`](validation/lan-headless-2026-09-26.log).
These timings describe this hardware and localhost run; physical LAN/Wi-Fi
latency, packet loss and different hardware have not been measured.

Desktop UI/match visual acceptance, final native/WASM builds and browser
regressions subsequently passed. See [the final acceptance report](LAN_ACCEPTANCE.md);
those checks remain distinct from this headless evidence.
