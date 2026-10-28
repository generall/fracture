//! Real sockets; run explicitly on systems that permit local TCP/UDP binding.
use sim::{
    Vec2,
    level::{Level, Team},
    matchplay::MatchConfig,
    multiplayer::PlayerInput,
    replica::Frame,
};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tank_net::{
    codec,
    discovery::Discovery,
    protocol::{ClientMessage, Input, InputDatagram, Lobby, ServerMessage},
    replication::Decoder,
    session::{Client, Host, HostCommand},
};
struct TestClient {
    client: Client,
    lobby: Option<Lobby>,
    decoder: Decoder,
    frame: Option<Frame>,
    snapshots: usize,
    errors: Vec<String>,
    map: bool,
    starts: usize,
    progress: Option<sim::matchplay::MatchProgress>,
    events: Vec<sim::Event>,
    replica: Option<sim::World>,
    pending_map: Option<sim::replica::Map>,
}
impl TestClient {
    fn new(client: Client) -> Self {
        Self {
            client,
            lobby: None,
            decoder: Decoder::default(),
            frame: None,
            snapshots: 0,
            errors: vec![],
            map: false,
            starts: 0,
            progress: None,
            events: vec![],
            replica: None,
            pending_map: None,
        }
    }
    fn poll(&mut self) {
        for message in self.client.poll().unwrap() {
            match message {
                ServerMessage::Lobby(lobby) => self.lobby = Some(lobby),
                ServerMessage::Started { map, progress } => {
                    self.decoder = Decoder::default();
                    self.map = true;
                    self.starts += 1;
                    self.frame = None;
                    self.pending_map = Some(map);
                    self.progress = Some(progress);
                    self.events.clear();
                    // The fixture explicitly chooses a regular tank for each match.
                    self.client
                        .send(ClientMessage::Input(Input {
                            sequence: self.starts as u32 - 1,
                            fire: 0,
                            special: 0,
                            deploy: self.starts as u32,
                            control: PlayerInput {
                                deploy: Some(sim::VehicleKind::Tank),
                                ..Default::default()
                            },
                        }))
                        .unwrap();
                }
                ServerMessage::Snapshot {
                    delta,
                    events,
                    progress,
                    ..
                } => {
                    let frame = self.decoder.decode(*delta).unwrap();
                    if let Some(map) = self.pending_map.take() {
                        self.replica = Some(sim::World::from_replica(map, frame.clone()));
                    } else {
                        self.replica.as_mut().unwrap().apply_replica(frame.clone());
                    }
                    assert!(self.replica.as_ref().unwrap().is_replica());
                    self.frame = Some(frame);
                    self.events.extend(events);
                    self.progress = Some(progress);
                    self.snapshots += 1;
                }
                ServerMessage::Error(e) => self.errors.push(e),
                _ => {}
            }
        }
    }
}
fn wait(clients: &mut [TestClient], timeout: Duration, condition: impl Fn(&[TestClient]) -> bool) {
    let deadline = Instant::now() + timeout;
    loop {
        for c in clients.iter_mut() {
            c.poll();
        }
        if condition(clients) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out: snapshots {:?}, errors {:?}, frames {:?}, event counts {:?}",
            clients.iter().map(|c| c.snapshots).collect::<Vec<_>>(),
            clients.iter().map(|c| &c.errors).collect::<Vec<_>>(),
            clients
                .iter()
                .map(|c| c.frame.as_ref().map(|f| (
                    f.stats.ticks,
                    f.team,
                    f.position,
                    f.charges,
                    f.plasma_remaining,
                    f.alive,
                    f.tanks.iter().map(|t| (t.id, t.health)).collect::<Vec<_>>()
                )))
                .collect::<Vec<_>>(),
            clients.iter().map(|c| c.events.len()).collect::<Vec<_>>()
        );
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
#[ignore = "requires local TCP and UDP sockets"]
fn six_clients_discover_ready_play_and_return_to_lobby() {
    let (host, local) = Host::bind(
        Level::default(),
        MatchConfig::default(),
        "Host",
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        Some(0),
    )
    .unwrap();
    let mut discovery = Discovery::on_port(host.discovery_port.unwrap()).unwrap();
    let mut clients = vec![TestClient::new(local)];
    for n in 1..6 {
        clients.push(TestClient::new(
            Client::connect(host.address, &format!("Player {n}")).unwrap(),
        ));
    }
    wait(&mut clients, Duration::from_secs(3), |c| {
        c.iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.players.len() == 6))
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        for c in &mut clients {
            c.poll();
        }
        if discovery
            .poll()
            .iter()
            .any(|(a, ad)| a.port() == host.address.port() && ad.players == 6)
        {
            break;
        }
        assert!(Instant::now() < deadline, "LAN discovery failed");
        thread::sleep(Duration::from_millis(10));
    }
    // Cannot start before everyone is ready.
    host.command(HostCommand::Start).unwrap();
    wait(&mut clients, Duration::from_secs(1), |c| {
        !c[0].errors.is_empty()
    });
    clients[0].errors.clear();
    for c in &mut clients {
        c.client
            .send(ClientMessage::Ready {
                revision: c.lobby.as_ref().unwrap().revision,
                ready: true,
            })
            .unwrap();
    }
    wait(&mut clients, Duration::from_secs(1), |c| {
        c[0].lobby.as_ref().unwrap().can_start()
    });
    host.command(HostCommand::Start).unwrap();
    wait(&mut clients, Duration::from_secs(5), |c| {
        c.iter()
            .all(|c| c.frame.as_ref().is_some_and(|f| f.stats.ticks >= 1))
    });
    let ids: std::collections::BTreeSet<_> = clients
        .iter()
        .map(|c| c.frame.as_ref().unwrap().player)
        .collect();
    assert_eq!(ids.len(), 6);
    let starts: Vec<_> = clients
        .iter()
        .map(|c| c.frame.as_ref().unwrap().position)
        .collect();
    for sequence in 1..=40 {
        for (index, c) in clients.iter_mut().enumerate() {
            c.client
                .send(ClientMessage::Input(Input {
                    sequence,
                    fire: 0,
                    special: 0,
                    deploy: 1,
                    control: PlayerInput {
                        tracks: if index == 2 { [1.0; 2] } else { [0.0; 2] },
                        aim: starts[index] + Vec2::Y * 20.0,
                        ..Default::default()
                    },
                }))
                .unwrap();
            c.poll();
        }
        thread::sleep(Duration::from_millis(20));
    }
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter()
            .all(|c| c.frame.as_ref().unwrap().stats.ticks >= 95)
    });
    assert!(
        clients[2]
            .frame
            .as_ref()
            .unwrap()
            .position
            .distance(starts[2])
            > 0.2
    );
    assert!(
        clients[4]
            .frame
            .as_ref()
            .unwrap()
            .position
            .distance(starts[4])
            < 0.05
    );
    assert!(clients.iter().all(|c| c.errors.is_empty() && c.map));
    // This guest stops consuming updates. Other peers and physics keep advancing.
    let mut slow = clients.pop().unwrap();
    let tick = clients[0].frame.as_ref().unwrap().stats.ticks;
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[0].frame.as_ref().unwrap().stats.ticks >= tick + 120
    });
    assert!(
        clients[2]
            .frame
            .as_ref()
            .unwrap()
            .tracks
            .iter()
            .all(|speed| speed.abs() < 0.2),
        "stale input must engage the brakes"
    );
    slow.client.send(ClientMessage::Leave).unwrap();
    drop(slow);
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[0].lobby.as_ref().unwrap().players.len() == 5
    });
    host.command(HostCommand::ReturnToLobby).unwrap();
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter().all(|c| !c.lobby.as_ref().unwrap().playing)
    });
    let previous = clients[0].lobby.as_ref().unwrap().revision;
    host.command(HostCommand::Configure {
        level: Box::new(Level::demo()),
        config: MatchConfig::default(),
    })
    .unwrap();
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter()
            .all(|c| c.lobby.as_ref().unwrap().revision > previous)
    });
    clients[1]
        .client
        .send(ClientMessage::Ready {
            revision: previous,
            ready: true,
        })
        .unwrap();
    wait(&mut clients, Duration::from_secs(1), |c| {
        !c[1].errors.is_empty()
    });
    assert!(
        clients[0]
            .lobby
            .as_ref()
            .unwrap()
            .players
            .iter()
            .all(|p| !p.ready)
    );
    let teams: Vec<_> = clients[0]
        .lobby
        .as_ref()
        .unwrap()
        .players
        .iter()
        .map(|p| p.team)
        .collect();
    assert!(teams.contains(&Team::Blue) && teams.contains(&Team::Red));
    println!(
        "Six TCP clients, UDP discovery, readiness, independent input, slow guest, disconnect and lobby reset passed. Traffic bytes: {:?}",
        clients
            .iter()
            .map(|c| c.client.traffic())
            .collect::<Vec<_>>()
    );
}

fn connect_players(level: Level, config: MatchConfig, count: usize) -> (Host, Vec<TestClient>) {
    let (host, local) = Host::bind(
        level,
        config,
        "Headless host",
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        None,
    )
    .unwrap();
    let mut clients = vec![TestClient::new(local)];
    for n in 1..count {
        clients.push(TestClient::new(
            Client::connect(host.address, &format!("Guest {n}")).unwrap(),
        ));
    }
    wait(&mut clients, Duration::from_secs(3), |clients| {
        clients
            .iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.players.len() == count))
    });
    (host, clients)
}
fn ready_and_start(host: &Host, clients: &mut [TestClient]) {
    let previous = clients[0].starts;
    for c in clients.iter_mut() {
        c.client
            .send(ClientMessage::Ready {
                revision: c.lobby.as_ref().unwrap().revision,
                ready: true,
            })
            .unwrap();
    }
    wait(clients, Duration::from_secs(2), |c| {
        c[0].lobby.as_ref().unwrap().can_start()
    });
    host.command(HostCommand::Start).unwrap();
    wait(clients, Duration::from_secs(5), |c| {
        c.iter()
            .all(|c| c.starts > previous && c.frame.as_ref().is_some_and(|f| f.stats.ticks > 0))
    });
}
#[test]
#[ignore = "requires local TCP sockets"]
fn time_limit_then_map_change_and_capture_rematch_reach_every_client() {
    let config = MatchConfig {
        time_limit_seconds: Some(1),
        airdrops: false,
        ..Default::default()
    };
    let (host, mut clients) = connect_players(Level::default(), config, 2);
    ready_and_start(&host, &mut clients);
    wait(&mut clients, Duration::from_secs(3), |c| {
        c.iter().all(|c| {
            c.progress.as_ref().unwrap().outcome == Some(sim::matchplay::Outcome::TimeLimit)
        })
    });
    for c in &clients {
        assert_eq!(c.progress.as_ref().unwrap().elapsed_ticks, 120);
        assert!(!c.lobby.as_ref().unwrap().config.airdrops);
        assert!(!c.progress.as_ref().unwrap().config.airdrops);
    }
    host.command(HostCommand::ReturnToLobby).unwrap();
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter().all(|c| !c.lobby.as_ref().unwrap().playing)
    });
    let mut capture_map = Level {
        name: "Capture rematch".into(),
        ..Default::default()
    };
    if let sim::level::LevelItem::Flag { position, .. } = &mut capture_map.items[2] {
        *position = [18.0, 0.0];
    }
    host.command(HostCommand::Configure {
        level: Box::new(capture_map),
        config: MatchConfig::default(),
    })
    .unwrap();
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter()
            .all(|c| c.lobby.as_ref().unwrap().map_name == "Capture rematch")
    });
    ready_and_start(&host, &mut clients);
    wait(&mut clients, Duration::from_secs(2), |c| {
        c.iter().all(|c| {
            c.progress.as_ref().unwrap().outcome
                == Some(sim::matchplay::Outcome::Captured(Team::Red))
        })
    });
    for c in &clients {
        assert_eq!(c.starts, 2);
        assert!(c.lobby.as_ref().unwrap().config.airdrops);
        assert!(c.progress.as_ref().unwrap().config.airdrops);
        assert_eq!(
            c.progress.as_ref().unwrap().elapsed_ticks,
            2,
            "deployment is accepted before the spawn tick"
        );
        assert!(c.events.iter().any(|e| matches!(
            e,
            sim::Event::FlagCaptured {
                team: Team::Red,
                ..
            }
        )));
    }
}
#[test]
#[ignore = "requires local TCP sockets"]
fn remote_plasma_charges_once_destroys_props_kills_and_respawns_host() {
    let mut level = Level::default();
    if let sim::level::LevelItem::Spawn { position, .. } = &mut level.items[0] {
        *position = [-6.0, 0.0];
    }
    if let sim::level::LevelItem::Spawn { position, .. } = &mut level.items[1] {
        *position = [6.0, 0.0];
    }
    level.items.push(sim::level::LevelItem::Pickup {
        weapon: sim::SpecialWeapon::Plasma,
        position: [6.0, 0.0],
        respawn_time: 60.0,
        charges: 1,
        cooldown: 0.0,
    });
    level.items.push(sim::level::LevelItem::Object {
        asset: Box::new(sim::LibraryPreset::Crate.asset()),
        position: [0.0, 0.0],
        angle: 0.0,
        cutouts: vec![],
    });
    let (host, mut clients) = connect_players(level, MatchConfig::default(), 2);
    ready_and_start(&host, &mut clients);
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[1].frame.as_ref().unwrap().charges == 1
    });
    let activated = clients[1].progress.as_ref().unwrap().elapsed_ticks;
    let command = Input {
        sequence: 1,
        fire: 0,
        special: 1,
        deploy: 1,
        control: PlayerInput {
            aim: Vec2::new(-6.0, 0.0),
            ..Default::default()
        },
    };
    clients[1]
        .client
        .send(ClientMessage::Input(command))
        .unwrap();
    // A repeated packet cannot activate a second special or skip the charge.
    clients[1]
        .client
        .send(ClientMessage::Input(command))
        .unwrap();
    wait(&mut clients, Duration::from_secs(1), |c| {
        c[1].frame.as_ref().unwrap().plasma_remaining > 0.0
    });
    assert!(clients[0].frame.as_ref().unwrap().alive);
    assert!(clients[1].events.iter().all(|e| !matches!(
        e,
        sim::Event::Fired {
            ammunition: sim::Ammunition::Plasma,
            ..
        }
    )));
    wait(&mut clients, Duration::from_secs(2), |c| {
        !c[0].frame.as_ref().unwrap().alive
            && c[1].events.iter().any(|e| {
                matches!(
                    e,
                    sim::Event::Fired {
                        ammunition: sim::Ammunition::Plasma,
                        ..
                    }
                )
            })
    });
    assert!(clients[0].progress.as_ref().unwrap().elapsed_ticks >= activated + 110);
    assert_eq!(
        clients[1]
            .events
            .iter()
            .filter(|e| matches!(
                e,
                sim::Event::Fired {
                    ammunition: sim::Ammunition::Plasma,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(clients[0].events.iter().any(|e| matches!(
        e,
        sim::Event::TankDestroyed {
            team: Team::Blue,
            ..
        }
    )));
    assert!(clients[1].events.iter().any(|e| matches!(
        e,
        sim::Event::DecorationDestroyed {
            kind: sim::DecorationKind::Crate,
            ..
        }
    )));
    assert_eq!(clients[1].frame.as_ref().unwrap().charges, 0);
    clients[0]
        .client
        .send(ClientMessage::Input(Input {
            sequence: 2,
            fire: 0,
            special: 0,
            deploy: 2,
            control: PlayerInput {
                deploy: Some(sim::VehicleKind::Tank),
                ..Default::default()
            },
        }))
        .unwrap();
    wait(&mut clients, Duration::from_secs(12), |c| {
        c[0].frame.as_ref().unwrap().alive
    });
    assert_eq!(clients[0].progress.as_ref().unwrap().tanks_lost, [1, 0]);
    assert!(clients[1].frame.as_ref().unwrap().alive);
}

#[test]
#[ignore = "requires local TCP sockets"]
fn backed_up_reader_skips_snapshots_without_stalling_others_and_recovers_deltas() {
    let (host, mut clients) = connect_players(Level::river_valley(), MatchConfig::default(), 2);
    let stream = std::net::TcpStream::connect(host.address).unwrap();
    let tuning = stream.try_clone().unwrap();
    clients.push(TestClient::new(
        Client::from_stream(stream, "Slow reader").unwrap(),
    ));
    wait(&mut clients, Duration::from_secs(3), |c| {
        c.iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.players.len() == 3))
    });
    ready_and_start(&host, &mut clients);
    socket2::SockRef::from(&tuning)
        .set_recv_buffer_size(1024)
        .unwrap();
    let mut slow = clients.pop().unwrap();
    let origin = slow.frame.as_ref().unwrap().position;
    let baseline = clients[0].frame.as_ref().unwrap().stats.ticks;
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut sequence = 0;
    loop {
        sequence += 1;
        slow.client
            .send(ClientMessage::Input(Input {
                sequence,
                fire: 0,
                special: 0,
                deploy: 1,
                control: PlayerInput {
                    aim: origin + Vec2::from_angle(sequence as f32 * 0.1) * 20.0,
                    ..Default::default()
                },
            }))
            .unwrap();
        for c in &mut clients {
            c.poll();
        }
        if host.metrics().skipped_snapshots >= 5
            && clients[0].frame.as_ref().unwrap().stats.ticks >= baseline + 240
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Did not exercise TCP backpressure: skipped {}, ticks {}",
            host.metrics().skipped_snapshots,
            clients[0].frame.as_ref().unwrap().stats.ticks
        );
        thread::sleep(Duration::from_millis(15));
    }
    let metrics = host.metrics();
    assert!(
        metrics.peak_queued_bytes > 0
            && metrics.peak_queued_bytes <= tank_net::codec::MAX_MESSAGE + 9
    );
    assert!(clients[1].frame.as_ref().unwrap().stats.ticks >= baseline + 200);
    let catch_up_tick = clients[0].frame.as_ref().unwrap().stats.ticks;
    socket2::SockRef::from(&tuning)
        .set_recv_buffer_size(256 * 1024)
        .unwrap();
    clients.push(slow);
    wait(&mut clients, Duration::from_secs(5), |c| {
        c.iter()
            .all(|c| c.frame.as_ref().unwrap().stats.ticks >= catch_up_tick)
    });
    assert!(clients.iter().all(|c| c.errors.is_empty()));
    println!(
        "Slow reader recovery: {} snapshots skipped, peak app queue {} bytes",
        metrics.skipped_snapshots, metrics.peak_queued_bytes
    );
}
#[test]
#[ignore = "requires local TCP sockets"]
fn late_joiners_enter_running_match_deploy_drive_and_reuse_departed_slots() {
    let config = MatchConfig {
        airdrops: false,
        ..Default::default()
    };
    let (host, mut clients) = connect_players(Level::default(), config, 2);
    ready_and_start(&host, &mut clients);
    wait(&mut clients, Duration::from_secs(3), |c| {
        c.iter()
            .all(|c| c.frame.as_ref().is_some_and(|f| f.stats.ticks >= 120))
    });
    clients.push(TestClient::new(
        Client::connect(host.address, "Latecomer").unwrap(),
    ));
    wait(&mut clients, Duration::from_secs(5), |c| {
        c[2].frame.as_ref().is_some_and(|f| f.alive)
            && c[0].lobby.as_ref().unwrap().players.len() == 3
    });
    let late = &clients[2];
    assert_eq!(late.starts, 1);
    assert!(late.errors.is_empty(), "{:?}", late.errors);
    assert!(late.progress.as_ref().unwrap().elapsed_ticks >= 120);
    let frame = late.frame.clone().unwrap();
    // Two players are split evenly, so the newcomer balances onto Blue.
    assert_eq!(frame.team, Team::Blue);
    assert!(
        frame.bodies.len() > 3,
        "the first snapshot carries the whole scene"
    );
    let lobby = clients[0].lobby.clone().unwrap();
    assert!(lobby.playing);
    assert_eq!(lobby.players[2].team, Team::Blue);
    let start = frame.position;
    for sequence in 1..=40 {
        clients[2]
            .client
            .send(ClientMessage::Input(Input {
                sequence,
                fire: 0,
                special: 0,
                deploy: 1,
                control: PlayerInput {
                    tracks: [1.0; 2],
                    aim: start + Vec2::Y * 20.0,
                    ..Default::default()
                },
            }))
            .unwrap();
        for c in &mut clients {
            c.poll();
        }
        thread::sleep(Duration::from_millis(20));
    }
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[2].frame
            .as_ref()
            .is_some_and(|f| f.position.distance(start) > 0.2)
    });
    let id = clients[2].frame.as_ref().unwrap().player;
    for c in &clients[..2] {
        let tank = c.frame.as_ref().unwrap().tanks.iter().find(|t| t.id == id);
        assert!(tank.is_some_and(|t| t.team == Team::Blue && t.health > 0.0));
    }
    // A departure frees both the session slot and the tank for the next guest.
    let tanks = clients[0].frame.as_ref().unwrap().tanks.len();
    clients
        .pop()
        .unwrap()
        .client
        .send(ClientMessage::Leave)
        .unwrap();
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[0].lobby.as_ref().unwrap().players.len() == 2
    });
    assert!(clients[0].frame.as_ref().unwrap().winner.is_none());
    clients.push(TestClient::new(
        Client::connect(host.address, "Second latecomer").unwrap(),
    ));
    wait(&mut clients, Duration::from_secs(5), |c| {
        c[2].frame.as_ref().is_some_and(|f| f.alive)
    });
    assert_eq!(clients[2].frame.as_ref().unwrap().player, id);
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[0].frame.as_ref().unwrap().tanks.len() == tanks
    });
    assert!(clients.iter().all(|c| c.errors.is_empty()));
}
/// Relays one guest's TCP connection. While `hold` is set, guest → host bytes
/// wait, as a Wi-Fi TCP stream does behind a lost segment's retransmission.
fn stalling_relay(host: SocketAddr, hold: Arc<AtomicBool>) -> SocketAddr {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut guest, _) = listener.accept().unwrap();
        let mut upstream = TcpStream::connect(host).unwrap();
        let (mut down_from, mut down_to) =
            (upstream.try_clone().unwrap(), guest.try_clone().unwrap());
        thread::spawn(move || {
            let _ = std::io::copy(&mut down_from, &mut down_to);
        });
        guest.set_nonblocking(true).unwrap();
        let mut buffer = [0u8; 4096];
        loop {
            if hold.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            match guest.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if upstream.write_all(&buffer[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        }
    });
    address
}
#[test]
#[ignore = "requires local TCP and UDP sockets"]
fn udp_input_keeps_steering_while_guest_tcp_stalls_and_rejects_forged_datagrams() {
    let config = MatchConfig {
        airdrops: false,
        ..Default::default()
    };
    let (host, mut clients) = connect_players(Level::default(), config, 1);
    let hold = Arc::new(AtomicBool::new(false));
    let relay = stalling_relay(host.address, hold.clone());
    clients.push(TestClient::new(
        Client::connect(relay, "Wi-Fi guest").unwrap(),
    ));
    wait(&mut clients, Duration::from_secs(3), |c| {
        c.iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.players.len() == 2))
    });
    ready_and_start(&host, &mut clients);
    wait(&mut clients, Duration::from_secs(5), |c| {
        c.iter().all(|c| c.frame.as_ref().is_some_and(|f| f.alive))
    });
    assert!(clients[1].client.has_input_channel());
    let guest_start = clients[1].frame.as_ref().unwrap().position;
    let host_start = clients[0].frame.as_ref().unwrap().position;
    let forger = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let drive = |sequence| Input {
        sequence,
        fire: 0,
        special: 0,
        deploy: 1,
        control: PlayerInput {
            tracks: [1.0; 2],
            aim: guest_start + Vec2::Y * 20.0,
            ..Default::default()
        },
    };
    hold.store(true, Ordering::Relaxed);
    let stalled = Instant::now();
    let mut sequence = 0;
    while stalled.elapsed() < Duration::from_millis(1500) {
        sequence += 1;
        clients[1].client.send_input(drive(sequence)).unwrap();
        // Another LAN device guessing the host player's slot cannot steer it.
        let forged = InputDatagram {
            slot: 0,
            token: 0x5eed,
            input: drive(sequence),
        };
        forger
            .send_to(
                &codec::encode(&forged).unwrap(),
                (Ipv4Addr::LOCALHOST, host.input_port.unwrap()),
            )
            .unwrap();
        for c in &mut clients {
            c.poll();
        }
        thread::sleep(Duration::from_millis(16));
    }
    // Every TCP input is still held, yet the guest kept driving past the
    // stale-input brake and the snapshots coming back show it.
    let guest = clients[1].frame.as_ref().unwrap();
    assert!(
        guest.position.distance(guest_start) > 1.0,
        "{guest_start} → {}",
        guest.position
    );
    assert!(
        guest.tracks.iter().all(|speed| *speed > 0.2),
        "{:?}",
        guest.tracks
    );
    let host_now = clients[0].frame.as_ref().unwrap().position;
    assert!(
        host_now.distance(host_start) < 0.05,
        "forged input moved the host"
    );
    hold.store(false, Ordering::Relaxed);
    let tick = clients[1].frame.as_ref().unwrap().stats.ticks;
    wait(&mut clients, Duration::from_secs(2), |c| {
        c[1].frame.as_ref().unwrap().stats.ticks > tick + 60
    });
    assert!(clients.iter().all(|c| c.errors.is_empty()));
}
