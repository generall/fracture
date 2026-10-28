//! Six real TCP clients on an authored map, including movement and live firing.
use sim::{Vec2, level::Level, matchplay::MatchConfig, multiplayer::PlayerInput};
use std::{
    net::{Ipv4Addr, SocketAddr},
    thread,
    time::{Duration, Instant},
};
use tank_net::{
    protocol::{ClientMessage, Input, ServerMessage},
    replication::Decoder,
    session::{Client, Host, HostCommand},
};
fn report(name: &str, samples: impl IntoIterator<Item = u32>) {
    let mut values: Vec<_> = samples.into_iter().collect();
    values.sort();
    if values.is_empty() {
        return;
    }
    let pct = |p: f32| values[((values.len() - 1) as f32 * p) as usize] as f32 / 1000.0;
    println!(
        "{name}: n={} median={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
        values.len(),
        pct(0.5),
        pct(0.95),
        pct(0.99),
        pct(1.0)
    );
}
fn main() {
    let level = if std::env::args().any(|a| a == "--quarry") {
        Level::demo()
    } else {
        Level::river_valley()
    };
    println!(
        "{}: {:?} metres, {} authored items",
        level.name,
        level.size,
        level.items.len()
    );
    let (host, local) = Host::bind(
        level,
        MatchConfig::default(),
        "Benchmark host",
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        None,
    )
    .unwrap();
    let mut clients = vec![local];
    for n in 1..6 {
        clients.push(Client::connect(host.address, &format!("Peer {n}")).unwrap());
    }
    let mut decoders: Vec<_> = (0..6).map(|_| Decoder::default()).collect();
    let mut ready = [false; 6];
    let mut sent_start = false;
    let start = Instant::now();
    let mut play_start = None;
    let mut last_input = Instant::now();
    let mut sequence = 0;
    let mut snapshots = [0; 6];
    let mut positions = [Vec2::ZERO; 6];
    let mut first_bytes = [0; 6];
    let mut snapshot_spacing: Vec<u32> = Vec::new();
    let mut last_snapshot = Instant::now();
    let mut bodies = [0; 6];
    loop {
        for (index, client) in clients.iter_mut().enumerate() {
            for message in client.poll().unwrap() {
                match message {
                    ServerMessage::Lobby(lobby) => {
                        if lobby.players.len() == 6 && !ready[index] {
                            client
                                .send(ClientMessage::Ready {
                                    revision: lobby.revision,
                                    ready: true,
                                })
                                .unwrap();
                            ready[index] = true;
                        }
                        if index == 0 && lobby.can_start() && !sent_start {
                            host.command(HostCommand::Start).unwrap();
                            sent_start = true;
                        }
                    }
                    ServerMessage::Snapshot {
                        delta, progress, ..
                    } => {
                        let frame = decoders[index].decode(*delta).unwrap();
                        snapshots[index] += 1;
                        positions[index] = frame.position;
                        bodies[index] = bodies[index].max(frame.bodies.len());
                        if index == 0 && frame.stats.ticks > 0 {
                            if play_start.is_some() {
                                snapshot_spacing.push(last_snapshot.elapsed().as_micros() as u32);
                            } else {
                                play_start = Some(Instant::now());
                            }
                            last_snapshot = Instant::now();
                        }
                        if progress.outcome.is_some() {
                            println!("Match ended at tick {}", frame.stats.ticks);
                        }
                    }
                    ServerMessage::Error(e) => panic!("Host rejected test: {e}"),
                    _ => {}
                }
            }
        }
        if let Some(playing) = play_start {
            if first_bytes[0] == 0 {
                for (i, c) in clients.iter().enumerate() {
                    first_bytes[i] = c.traffic().1;
                }
            }
            if playing.elapsed() > Duration::from_secs(10) {
                break;
            }
            if last_input.elapsed() >= Duration::from_millis(16) {
                sequence += 1;
                last_input = Instant::now();
                for (i, c) in clients.iter_mut().enumerate() {
                    c.send_input(Input {
                        sequence,
                        fire: sequence / 60,
                        special: 0,
                        deploy: 1,
                        control: PlayerInput {
                            deploy: Some(sim::VehicleKind::Tank),
                            tracks: [0.75, 0.75],
                            aim: Vec2::new(0.0, ((i as f32) * 1.5).sin() * 15.0),
                            ..Default::default()
                        },
                    })
                    .unwrap();
                }
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(45),
            "Host failed to start"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let duration = play_start.unwrap().elapsed().as_secs_f64();
    let metrics = host.metrics();
    println!(
        "Peak authoritative bodies: {}, snapshots skipped for backpressure: {}",
        metrics.peak_bodies, metrics.skipped_snapshots
    );
    report("Host physics tick", metrics.tick_us);
    report("All six snapshots", metrics.snapshot_us);
    report("Client snapshot interval", snapshot_spacing);
    println!(
        "Simulated ticks: {}, catch-up caps: {}, snapshots per peer: {:?}, peak visible bodies: {:?}",
        metrics.ticks, metrics.capped_catchups, snapshots, bodies
    );
    for (i, c) in clients.iter().enumerate() {
        println!(
            "Peer {i}: {:.1} KiB/s received, position {:?}",
            (c.traffic().1 - first_bytes[i]) as f64 / duration / 1024.0,
            positions[i]
        );
    }
}
