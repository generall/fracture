# LAN multiplayer

Desktop LAN multiplayer supports one host and up to five guests. Local
single-player, sandbox and browser play remain separate options. Headless,
desktop and browser acceptance checks passed; see [validation](LAN_ACCEPTANCE.md).

## Run and join

Build and run the desktop game on each computer:

```sh
cargo run --release --locked -p game
```

Choose **Play on local network**. Enter a player name. One player chooses **Host a
lobby**; guests choose the discovered lobby or enter the host computer's IPv4
address and choose **Join by IP**. `127.0.0.1` is useful only for another instance
on the same computer. A direct address may include `:port`.

The host selects River Valley, Classic Tanks 2 or an imported JSON map, and
chooses the match duration. Map creation is available in the separate sandbox. Everyone chooses a team and marks themselves ready. Both teams
need a player; the host starts after all players are ready. Changing the map or
match settings clears readiness. Changing a player's team clears that player's
readiness.

Guests can also join a match that is already running: choose a discovered lobby
marked **IN MATCH / JOIN**, or join by IP as usual. The newcomer goes to the team
with fewer players, receives the map and the current battlefield (including
destruction so far), chooses a vehicle, and deploys at that team's spawn. A
departed player's tank is reused, so a session never exceeds six players. Other
players see a short notice when someone joins or leaves.

The countdown lasts three seconds. Capture the enemy flag to win; destroyed tanks
require a vehicle selection after every death and respawn after a minimum of ten seconds when their spawn is clear. Initial deployment also offers Scout Car, Regular Tank and Mortar. The host can return the
session to the lobby for another map or match. A guest can leave independently.
The host leaving closes the session. Match menus and sound settings do not pause
the shared simulation; that player's movement input is neutral while a menu is open.

Default ports are **TCP 45771** for the session, **UDP 45771** for input and
**UDP 45770** for discovery. If a firewall blocks UDP 45771, play still works:
input then travels over TCP only and is more sensitive to Wi-Fi hiccups.
Discovery uses IPv4 broadcast and a loopback query. IP joining works when broadcasts
are unavailable. This is LAN play, without internet matchmaking, NAT traversal or host
migration. Browser builds retain local play;
raw LAN sockets are desktop-only.

## Authority and wire format

- A dedicated host thread advances the complete simulation at 120 Hz, with the
  existing four physics microsteps. Every human has independent weapon state.
- Clients send validated controls, aim, ammunition selection and monotonic action
  counters. They cannot submit tank positions, shell energy, damage or ownership.
  Stale movement brakes after 500 ms; accepted fire/special/deploy actions execute
  once, even after a scheduling stall. Connections time out after ten seconds.
- Every input is sent twice: as a UDP datagram and over TCP. The host applies
  whichever copy arrives first and rejects older sequences, so a TCP stream waiting
  on a retransmission no longer holds up steering. Actions are cumulative counters
  in every input, so a lost or overtaken copy loses nothing. Datagrams carry a
  per-connection random token and must come from the TCP peer's address.
- Clients contain presentation replicas with no physics bodies. A replica cannot
  advance physics. They interpolate between consecutive host snapshots, including
  tanks and turrets, and snap large respawn teleports instead of traversing the map.
- A jitter buffer presents snapshots on a smoothly advancing clock, using the
  host's send timestamps. Its delay is the typical snapshot spacing plus the 95th
  percentile of lateness over the last three seconds (at most 300 ms), rather than
  the worst stall, which would add that stall's length to all motion. When a
  snapshot is late anyway, motion is extrapolated for up to 100 ms, then held. The
  clock changes speed by at most 15% to adapt and never steps backwards; it only
  jumps forward after a gap longer than half a second, such as a host pause.
- A versioned binary protocol uses varint bincode and LZ4 when compression saves
  space. Each frame has bounded encoded and expanded lengths (8 MiB). Immutable
  terrain/flag data is transferred once per match.
- The host sends snapshots at 30 Hz. Persistent geometry definitions are sent on
  first observation or change; motion is quantized to millimetres, approximately
  0.0001 radians and 0.01 m/s, and only changed records are sent. Unchanged vision
  polygons are omitted. Geometry caches survive temporary loss of sight.
- Each player receives whole visible physical figures, detected tanks, visible
  projectiles, effects, pickups and scenery updates. Hidden figures stop updating.
  The existing client fog retains last-observed scenery. Fracture ancestry keeps
  remembered figures consistent when visible chunks subdivide.
- TCP is nonblocking with Nagle disabled, bounded kernel/application send buffers,
  and per-poll work limits. On Linux and macOS, `TCP_NOTSENT_LOWAT` bounds only
  *unsent* kernel data (16 KiB), so in-flight data can follow the congestion window
  (up to a 128 KiB buffer) on higher-latency Wi-Fi without obsolete snapshots
  queueing; elsewhere the whole kernel buffer stays at 32 KiB.
- Gameplay sockets set DSCP CS4 (snapshots) and EF (input datagrams). Wi-Fi maps
  these to the WMM video and voice access categories (RFC 8325), which contend for
  airtime ahead of bulk traffic. Networks that ignore DSCP are unaffected. A backed-up peer skips new snapshots before delta
  generation, keeping its reliable delta chain valid while other peers continue.
  Control messages remain ordered with map/snapshot updates.
- Blocker classification, collider bounds and composed grass are prepared once
  per snapshot batch and reused for all players. The cache expires before the next
  physics tick. Shared geometry and diagnostics are captured once per batch too.

Snapshots stay on TCP: destructive geometry deltas need reliable, ordered
delivery, and the jitter buffer absorbs the occasional retransmission delay.
Input, which is small and superseded every frame, uses UDP as well.

## Wi-Fi

Prefer 5 GHz Wi-Fi or a wired host. Many laptops and the Steam Deck save power by
letting the Wi-Fi radio doze between beacons, which delays arriving packets by
100 ms or more. If play still stutters, disable Wi-Fi power saving (Steam Deck:
**Settings → Developer → Enable Wi-Fi Power Management** off; Linux:
`iw dev <interface> set power_save off`).

## Headless verification

```sh
bash tools/check-lan-headless.sh
```

The script unsets display variables, runs workspace tests, explicitly runs the
socket integration tests, checks lint, and runs a ten-second six-client River
Valley movement/firing benchmark. Socket tests are ignored by the ordinary
workspace command because restricted build environments may prohibit TCP/UDP
binding. They are explicitly included by this script.

Individual commands:

```sh
cargo test -p tank-net --locked --test lan -- --ignored --nocapture
cargo run --release --locked -p tank-net --example lan_benchmark
cargo run --release --locked -p tank-net --example replication_profile
```

`lan_benchmark --quarry` selects the smaller Quarry Crossing map. The benchmark
reports physics and snapshot timing percentiles, received bandwidth for each
client, visible-body counts, and dropped catch-up time. These are actual localhost
sockets and protocol decoding; no graphics or audio device is opened.

See [the headless report](LAN_HEADLESS_TESTS.md) for measured results and
[the acceptance report](LAN_ACCEPTANCE.md) for desktop and browser checks.
Physical multi-computer LAN/Wi-Fi performance has not been measured.
