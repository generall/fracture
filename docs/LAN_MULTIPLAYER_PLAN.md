# LAN multiplayer implementation and acceptance

User objective: a host-authoritative local-network game for up to six players,
with IP joining and discovery, map selection, team/ready lobby, and efficient
visibility-filtered binary replication. Target: the desktop game; existing local
and browser play must keep working.

## Required implementation

- A host session owns all physics, combat, pickups, timers and match outcomes.
  Each of up to six human tanks has independent input, weapons and respawn state.
- Direct address joining and UDP LAN discovery. Bound parsing, timeouts, input
  ownership and outbound buffering; slow/disconnected clients cannot stall physics.
- A lobby shows names, team assignments and readiness. Host alone selects/edits
  maps/configuration and starts when all connected players are ready. Map changes
  invalidate readiness. Join failures and disconnects are surfaced in the UI.
- Binary, compressed game updates with persistent geometry and changed-state
  deltas. Per-player visibility filters dynamic state; clients retain appropriate
  last-seen scenery without receiving hidden changes. Reliable session/control
  messages and stale-input neutralization. Measure bandwidth and tick cost.
- Clients send controls, never authoritative positions or damage. Clients render
  interpolated snapshots with the existing visuals, HUD and sound; no client
  physics stepping. Local menus/settings must not pause the server for others.
- Start, play, capture/time-limit results, leave/disconnect and another match all
  work. The local single-player and sandbox/editor paths remain available.

## Completion evidence to collect

- Unit/integration tests for six independent tanks, combat/specials/respawns,
  serialization/deltas, visibility privacy/re-entry, malformed input, readiness,
  map transfer, disconnects and bounded slow-peer behavior.
- Real loopback host plus five client connections, discovery and direct joins,
  readiness/start, input/snapshot exchange and match lifecycle.
- A large authored map with six clients: record host tick timings, snapshot sizes,
  bandwidth and interpolation behavior; resolve measured bottlenecks.
- Desktop UI integration check for host/join/lobby/gameplay and existing browser
  regression checks. Native and WASM builds, formatting, workspace tests and lint.
- User documentation with run/join instructions, ports/protocol, performance
  measurements and any verified limitations. Audit every requirement above before
  marking the goal complete.

Status: implemented and verified on 2026-09-26. The source, tests and measured
runtime results are the completion evidence; see [the final audit](LAN_ACCEPTANCE.md).

The user requested headless testing first. The headless suite passed before the
remaining desktop checks: 250 unit tests, four real-socket scenarios, formatting,
lint and the six-client River Valley benchmark. Native/WASM builds, browser game
and audio regressions, and two-client desktop acceptance subsequently passed.
Physical multi-computer network performance remains unmeasured, not implied by
the local socket benchmark.
