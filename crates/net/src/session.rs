//! Dedicated host simulation thread and a common TCP client for host and guests.
use crate::{
    codec, discovery,
    protocol::*,
    replication::{self, Encoder},
    transport::{self, Connection},
};
use sim::{
    level::{Level, Team},
    matchplay::{MatchConfig, MatchProgress},
    multiplayer::{MultiplayerWorld, PlayerInput},
    replica::{Frame, Map},
};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Every fourth 120 Hz tick; clients buffer and interpolate between snapshots.
const SNAPSHOT_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 30);
/// Movement brakes when no input arrives for this long. Long enough to ride
/// out a typical Wi-Fi latency spike instead of stopping and lurching.
const STALE_INPUT: Duration = Duration::from_millis(500);

pub struct Client {
    connection: Connection,
    /// Unreliable copies of input, with this client's slot and token.
    inputs: Option<(UdpSocket, u8, u64)>,
    last_ping: Instant,
    last_received: Instant,
}
impl Client {
    pub fn connect(address: SocketAddr, name: &str) -> io::Result<Self> {
        Self::from_stream(
            TcpStream::connect_timeout(&address, Duration::from_secs(3))?,
            name,
        )
    }
    /// Join using an already connected stream (also useful for socket tuning).
    pub fn from_stream(stream: TcpStream, name: &str) -> io::Result<Self> {
        if !valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Player name must be 1–32 characters without control characters",
            ));
        }
        let mut connection = Connection::new(stream)?;
        connection.send(&ClientMessage::Hello {
            version: VERSION,
            name: name.trim().into(),
        })?;
        connection.flush()?;
        Ok(Self {
            connection,
            inputs: None,
            last_ping: Instant::now(),
            last_received: Instant::now(),
        })
    }
    pub fn send(&mut self, message: ClientMessage) -> io::Result<()> {
        self.connection.send(&message)?;
        self.connection.flush()
    }
    /// Sends controls over UDP and TCP; the host keeps whichever arrives
    /// first. A lost datagram costs nothing, and a TCP retransmission stall
    /// no longer delays steering while datagrams get through.
    pub fn send_input(&mut self, input: Input) -> io::Result<()> {
        if let Some((socket, slot, token)) = &self.inputs {
            let datagram = InputDatagram {
                slot: *slot,
                token: *token,
                input,
            };
            if let Ok(bytes) = codec::encode(&datagram) {
                let _ = socket.send(&bytes);
            }
        }
        self.send(ClientMessage::Input(input))
    }
    /// True once the host has offered, and this client opened, the UDP channel.
    pub fn has_input_channel(&self) -> bool {
        self.inputs.is_some()
    }
    fn open_input_channel(&self, port: u16) -> io::Result<UdpSocket> {
        let host = SocketAddr::new(self.connection.peer_addr()?.ip(), port);
        let any: IpAddr = if host.is_ipv4() {
            Ipv4Addr::UNSPECIFIED.into()
        } else {
            Ipv6Addr::UNSPECIFIED.into()
        };
        let socket = UdpSocket::bind((any, 0))?;
        socket.connect(host)?;
        socket.set_nonblocking(true)?;
        transport::mark((&socket).into(), transport::INPUT_TOS);
        Ok(socket)
    }
    pub fn poll(&mut self) -> io::Result<Vec<ServerMessage>> {
        if self.last_ping.elapsed() >= Duration::from_secs(1) {
            self.connection.send(&ClientMessage::Ping)?;
            self.last_ping = Instant::now();
        }
        self.connection.flush()?;
        let messages = self.connection.receive()?;
        if !messages.is_empty() {
            self.last_received = Instant::now();
        }
        for message in &messages {
            if let ServerMessage::Welcome {
                slot,
                input: Some(channel),
            } = message
            {
                // Without the channel, input still travels reliably over TCP.
                self.inputs = self
                    .open_input_channel(channel.port)
                    .ok()
                    .map(|socket| (socket, *slot, channel.token));
            }
        }
        if self.last_received.elapsed() > Duration::from_secs(10) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Host stopped responding",
            ));
        }
        Ok(messages)
    }
    pub fn traffic(&self) -> (u64, u64) {
        (self.connection.sent, self.connection.received)
    }
}
pub enum HostCommand {
    Configure {
        level: Box<Level>,
        config: MatchConfig,
    },
    Start,
    ReturnToLobby,
}
/// Bounded timing samples for diagnostics and reproducible LAN capacity checks.
#[derive(Clone, Default)]
pub struct Metrics {
    pub ticks: u64,
    pub tick_us: VecDeque<u32>,
    pub snapshot_us: VecDeque<u32>,
    pub capped_catchups: u64,
    pub skipped_snapshots: u64,
    pub peak_queued_bytes: usize,
    pub peak_bodies: usize,
}
impl Metrics {
    fn sample(values: &mut VecDeque<u32>, start: Instant) {
        if values.len() == 4096 {
            values.pop_front();
        }
        values.push_back(start.elapsed().as_micros().min(u32::MAX as u128) as u32);
    }
}
pub struct Host {
    metrics: Arc<Mutex<Metrics>>,
    pub address: SocketAddr,
    pub discovery_port: Option<u16>,
    /// UDP port for input datagrams, if it could be opened.
    pub input_port: Option<u16>,
    commands: SyncSender<HostCommand>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Host {
    pub fn start(level: Level, config: MatchConfig, name: &str) -> Result<(Self, Client), String> {
        Self::bind(
            level,
            config,
            name,
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT)),
            Some(DISCOVERY_PORT),
        )
    }
    pub fn bind(
        level: Level,
        config: MatchConfig,
        name: &str,
        address: SocketAddr,
        discovery_port: Option<u16>,
    ) -> Result<(Self, Client), String> {
        level.validate()?;
        validate_config(config)?;
        if !valid_name(name) {
            return Err("Choose a player name of 1–32 characters.".into());
        }
        let listener =
            TcpListener::bind(address).map_err(|e| format!("Cannot host on {address}: {e}"))?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        // Reserve the host's connection before accepting any LAN guests. Both use
        // exactly the same replication path; host-only commands stay in-process.
        let local = SocketAddr::new(
            if address.ip().is_unspecified() {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            } else {
                address.ip()
            },
            address.port(),
        );
        let local_stream = TcpStream::connect(local).map_err(|e| e.to_string())?;
        let (host_stream, _) = listener.accept().map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let discovery = discovery_port
            .map(|port| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)))
            .transpose()
            .map_err(|e| format!("Cannot open LAN discovery: {e}"))?;
        if let Some(socket) = &discovery {
            socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        }
        let discovery_port = discovery
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .map(|a| a.port());
        // Input datagrams share the session's port number where it is free.
        // Without them, guests fall back to TCP input alone.
        let inputs = UdpSocket::bind(address)
            .or_else(|_| UdpSocket::bind(SocketAddr::new(address.ip(), 0)))
            .and_then(|socket| socket.set_nonblocking(true).map(|_| socket))
            .ok();
        let input_port = inputs
            .as_ref()
            .and_then(|s| s.local_addr().ok())
            .map(|a| a.port());
        let mut peers = BTreeMap::new();
        peers.insert(0, Peer::new(host_stream).map_err(|e| e.to_string())?);
        let lobby = Lobby {
            revision: 1,
            name: format!("{}'s game", name.trim()),
            map_name: level.name.clone(),
            map_size: level.size,
            map_objects: level.items.len(),
            config,
            players: vec![],
            playing: false,
        };
        let (commands, rx) = mpsc::sync_channel(8);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let worker_metrics = metrics.clone();
        let worker = thread::Builder::new()
            .name("lan-host".into())
            .spawn(move || {
                let mut server = Server {
                    metrics: worker_metrics,
                    listener,
                    discovery,
                    inputs,
                    epoch: Instant::now(),
                    port: address.port(),
                    peers,
                    lobby,
                    level,
                    game: None,
                    map: None,
                    progress: MatchProgress::new(config),
                    events: vec![],
                    next_tick: Instant::now(),
                    next_snapshot: Instant::now(),
                    dirty: true,
                };
                while !worker_stop.load(Ordering::Relaxed) {
                    for command in rx.try_iter().take(8) {
                        server.command(command);
                    }
                    if !server.poll() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            })
            .map_err(|e| e.to_string())?;
        let client = Client::from_stream(local_stream, name).map_err(|e| e.to_string())?;
        Ok((
            Self {
                metrics,
                address,
                discovery_port,
                input_port,
                commands,
                stop,
                thread: Some(worker),
            },
            client,
        ))
    }
    pub fn metrics(&self) -> Metrics {
        self.metrics.lock().unwrap().clone()
    }
    pub fn command(&self, command: HostCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|e| format!("Host is busy: {e}"))
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn validate_config(config: MatchConfig) -> Result<(), String> {
    if config
        .time_limit_seconds
        .is_some_and(|s| s == 0 || s > 3600)
    {
        Err("Match duration must be 1–3600 seconds or unlimited.".into())
    } else {
        Ok(())
    }
}
struct Peer {
    connection: Connection,
    /// Authenticates this peer's input datagrams, which must also come from
    /// the TCP peer's address.
    token: u64,
    ip: Option<IpAddr>,
    player: Option<Player>,
    index: Option<usize>,
    last_seen: Instant,
    controls: ControlState,
    encoder: Encoder,
}
impl Peer {
    fn new(stream: TcpStream) -> io::Result<Self> {
        Ok(Self {
            ip: stream.peer_addr().ok().map(|a| a.ip()),
            connection: Connection::new(stream)?,
            token: token(),
            player: None,
            index: None,
            last_seen: Instant::now(),
            controls: ControlState::new(),
            encoder: Encoder::default(),
        })
    }
    /// Enter a match (at its start or already running) as `index`. A fresh
    /// encoder makes the first snapshot carry every body definition.
    fn enter(
        &mut self,
        index: usize,
        game: &mut MultiplayerWorld,
        map: &Map,
        progress: &MatchProgress,
    ) {
        self.index = Some(index);
        self.encoder = Encoder::default();
        let view = game.for_player(index).unwrap();
        self.controls.input = PlayerInput {
            aim: view.launcher_position() + sim::Vec2::from_angle(view.player_aim().angle) * 20.0,
            ..Default::default()
        };
        let mut progress = progress.clone();
        progress.config.team = view.team();
        let _ = self.connection.send(&ServerMessage::Started {
            map: map.clone(),
            progress,
        });
    }
}
/// Not cryptographic: it keeps stray or forged LAN datagrams off other slots.
fn token() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    hasher.finish()
}
struct ControlState {
    last_input: Instant,
    input: PlayerInput,
    sequence: Option<u32>,
    fire: u32,
    special: u32,
    deploy: u32,
}
impl ControlState {
    fn new() -> Self {
        Self {
            last_input: Instant::now(),
            input: PlayerInput::default(),
            sequence: None,
            fire: 0,
            special: 0,
            deploy: 0,
        }
    }
    fn take(&mut self) -> PlayerInput {
        let mut input = self.input;
        self.input.fire = false;
        self.input.special = false;
        self.input.deploy = None;
        if self.last_input.elapsed() > STALE_INPUT {
            input.tracks = [0.0; 2];
            // Brake stale movement, but deliver an accepted one-shot action once.
            // A host scheduling stall must not silently discard reliable input.
        }
        input
    }
    fn receive_input(&mut self, input: Input) {
        if !input.control.valid() {
            return;
        }
        if self.sequence.is_some_and(|old| {
            input.sequence.wrapping_sub(old) == 0 || input.sequence.wrapping_sub(old) > u32::MAX / 2
        }) {
            return;
        }
        let fire_delta = input.fire.wrapping_sub(self.fire);
        let special_delta = input.special.wrapping_sub(self.special);
        let deploy_delta = input.deploy.wrapping_sub(self.deploy);
        if [fire_delta, special_delta, deploy_delta]
            .iter()
            .any(|&delta| delta > u32::MAX / 2)
        {
            return;
        }
        let fire = self.input.fire || fire_delta > 0;
        let special = self.input.special || special_delta > 0;
        let deploy = input
            .control
            .deploy
            .filter(|_| deploy_delta > 0)
            .or(self.input.deploy);
        self.input = input.control;
        self.input.deploy = deploy;
        self.input.fire = fire;
        self.input.special = special;
        self.sequence = Some(input.sequence);
        self.fire = input.fire;
        self.special = input.special;
        self.deploy = input.deploy;
        self.last_input = Instant::now();
    }
}
struct Server {
    metrics: Arc<Mutex<Metrics>>,
    listener: TcpListener,
    discovery: Option<UdpSocket>,
    inputs: Option<UdpSocket>,
    /// Origin of the snapshot send timestamps.
    epoch: Instant,
    port: u16,
    peers: BTreeMap<u8, Peer>,
    lobby: Lobby,
    level: Level,
    game: Option<MultiplayerWorld>,
    /// Immutable terrain of the running match, resent to late joiners.
    map: Option<Map>,
    progress: MatchProgress,
    events: Vec<sim::Event>,
    next_tick: Instant,
    next_snapshot: Instant,
    dirty: bool,
}
impl Server {
    fn error(&mut self, slot: u8, message: impl Into<String>) {
        if let Some(peer) = self.peers.get_mut(&slot) {
            let _ = peer.connection.send(&ServerMessage::Error(message.into()));
        }
    }
    fn roster(&mut self) {
        self.lobby.players = self
            .peers
            .values()
            .filter_map(|p| p.player.clone())
            .collect();
    }
    fn command(&mut self, command: HostCommand) {
        match command {
            HostCommand::Configure { level, config } => {
                if self.game.is_some() {
                    self.error(0, "Return to the lobby before changing the map.");
                    return;
                }
                if let Err(e) = level.validate().and_then(|_| validate_config(config)) {
                    self.error(0, e);
                    return;
                }
                self.lobby.revision += 1;
                self.lobby.map_name = level.name.clone();
                self.lobby.map_size = level.size;
                self.lobby.map_objects = level.items.len();
                self.lobby.config = config;
                self.level = *level;
                for p in self.peers.values_mut().filter_map(|p| p.player.as_mut()) {
                    p.ready = false;
                }
                self.dirty = true;
            }
            HostCommand::Start => {
                self.roster();
                if !self.lobby.can_start() {
                    self.error(
                        0,
                        "Both teams need players, and every player must be ready.",
                    );
                    return;
                }
                let teams: Vec<_> = self.lobby.players.iter().map(|p| p.team).collect();
                let mut game = match MultiplayerWorld::new(&self.level, &teams) {
                    Ok(g) => g,
                    Err(e) => {
                        self.error(0, e);
                        return;
                    }
                };
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                game.configure_airdrops(self.lobby.config.airdrops, seed);
                game.require_vehicle_choices();
                let map = Map::capture(game.world());
                self.progress = MatchProgress::new(self.lobby.config);
                for (index, slot) in self.lobby.players.iter().map(|p| p.slot).enumerate() {
                    let peer = self.peers.get_mut(&slot).unwrap();
                    peer.enter(index, &mut game, &map, &self.progress);
                }
                self.game = Some(game);
                self.map = Some(map);
                self.lobby.playing = true;
                self.events.clear();
                self.next_tick = Instant::now() + Duration::from_secs(3);
                self.next_snapshot = Instant::now();
                self.dirty = true;
            }
            HostCommand::ReturnToLobby => {
                self.game = None;
                self.map = None;
                self.lobby.playing = false;
                self.lobby.revision += 1;
                self.events.clear();
                for peer in self.peers.values_mut() {
                    peer.index = None;
                    if let Some(p) = &mut peer.player {
                        p.ready = false;
                    }
                }
                self.dirty = true;
            }
        }
    }
    fn message(&mut self, slot: u8, message: ClientMessage) -> bool {
        let registered = self.peers[&slot].player.is_some();
        match message {
            ClientMessage::Hello { version, name } => {
                if registered || version != VERSION || !valid_name(&name) {
                    self.error(slot, "Cannot join: incompatible version or invalid name.");
                    return false;
                }
                let blue = self
                    .peers
                    .values()
                    .filter_map(|p| p.player.as_ref())
                    .filter(|p| p.team == Team::Blue)
                    .count();
                let red = self
                    .peers
                    .values()
                    .filter_map(|p| p.player.as_ref())
                    .filter(|p| p.team == Team::Red)
                    .count();
                let team = if blue <= red { Team::Blue } else { Team::Red };
                // A running match takes the newcomer on the smaller team at once.
                let index = match &mut self.game {
                    Some(game) => match game.join(team) {
                        Ok(index) => Some(index),
                        Err(e) => {
                            self.error(slot, format!("Cannot join: {e}"));
                            return false;
                        }
                    },
                    None => None,
                };
                let peer = self.peers.get_mut(&slot).unwrap();
                peer.player = Some(Player {
                    slot,
                    name: name.trim().into(),
                    team,
                    ready: false,
                });
                let input = self
                    .inputs
                    .as_ref()
                    .and_then(|socket| socket.local_addr().ok())
                    .map(|address| InputChannel {
                        port: address.port(),
                        token: peer.token,
                    });
                let _ = peer
                    .connection
                    .send(&ServerMessage::Welcome { slot, input });
                if let (Some(index), Some(game), Some(map)) = (index, &mut self.game, &self.map) {
                    peer.enter(index, game, map, &self.progress);
                }
                self.dirty = true;
            }
            ClientMessage::Leave => return false,
            _ if !registered => return false,
            ClientMessage::Team(team) if self.game.is_none() && team != Team::Neutral => {
                let p = self.peers.get_mut(&slot).unwrap().player.as_mut().unwrap();
                p.team = team;
                p.ready = false;
                self.dirty = true;
            }
            ClientMessage::Ready { revision, ready } if self.game.is_none() => {
                if revision == self.lobby.revision {
                    self.peers
                        .get_mut(&slot)
                        .unwrap()
                        .player
                        .as_mut()
                        .unwrap()
                        .ready = ready;
                    self.dirty = true;
                } else {
                    self.error(slot, "Map changed. Review the lobby and ready again.");
                }
            }
            ClientMessage::Input(input) if self.game.is_some() => self
                .peers
                .get_mut(&slot)
                .unwrap()
                .controls
                .receive_input(input),
            ClientMessage::Ping => {
                let _ = self
                    .peers
                    .get_mut(&slot)
                    .unwrap()
                    .connection
                    .send(&ServerMessage::Pong);
            }
            _ => {}
        }
        true
    }
    /// Applies the newest authenticated input datagrams. Stale, duplicate or
    /// reordered copies are rejected by the same sequence check as TCP input.
    fn receive_datagrams(&mut self) {
        let Some(socket) = &self.inputs else {
            return;
        };
        let mut data = [0u8; 512];
        for _ in 0..64 {
            let Ok((n, from)) = socket.recv_from(&mut data) else {
                break;
            };
            let Ok(Some((datagram, used))) = codec::decode::<InputDatagram>(&data[..n]) else {
                continue;
            };
            if let Some(peer) = self.peers.get_mut(&datagram.slot)
                && used == n
                && peer.token == datagram.token
                && peer.index.is_some()
                && peer.ip == Some(from.ip())
            {
                peer.controls.receive_input(datagram.input);
            }
        }
    }
    fn remove(&mut self, slot: u8) {
        if let Some(mut peer) = self.peers.remove(&slot) {
            let _ = peer.connection.flush();
            if let (Some(game), Some(index)) = (&mut self.game, peer.index) {
                game.disconnect(index);
            }
            self.dirty = true;
        }
    }
    fn poll(&mut self) -> bool {
        for _ in 0..8 {
            let Ok((stream, _)) = self.listener.accept() else {
                break;
            };
            let slot = (1..6).find(|slot| !self.peers.contains_key(slot));
            if let Some(slot) = slot {
                if let Ok(peer) = Peer::new(stream) {
                    self.peers.insert(slot, peer);
                }
            } else if let Ok(mut c) = Connection::new(stream) {
                let _ = c.send(&ServerMessage::Error("Lobby is full.".into()));
                let _ = c.flush();
            }
        }
        let mut messages = Vec::new();
        let mut dead = Vec::new();
        for (&slot, peer) in &mut self.peers {
            let timeout = if peer.player.is_none() {
                Duration::from_secs(3)
            } else {
                Duration::from_secs(10)
            };
            if peer.last_seen.elapsed() > timeout {
                dead.push(slot);
                continue;
            }
            match peer.connection.receive::<ClientMessage>() {
                Ok(batch) => {
                    if !batch.is_empty() {
                        peer.last_seen = Instant::now();
                    }
                    messages.extend(batch.into_iter().map(|m| (slot, m)));
                }
                Err(_) => {
                    dead.push(slot);
                }
            }
        }
        for (slot, message) in messages {
            if !dead.contains(&slot) && !self.message(slot, message) {
                dead.push(slot);
            }
        }
        for slot in dead {
            self.remove(slot);
        }
        if !self.peers.contains_key(&0) {
            return false;
        }
        self.receive_datagrams();
        if self.dirty {
            self.roster();
            let lobby = ServerMessage::Lobby(self.lobby.clone());
            for peer in self.peers.values_mut().filter(|p| p.player.is_some()) {
                let _ = peer.connection.send(&lobby);
            }
            self.dirty = false;
        }
        if let Some(socket) = &self.discovery {
            discovery::respond(socket, self.port, &self.lobby);
        }
        if let Some(game) = &mut self.game {
            let tick = Duration::from_secs_f64(1.0 / 120.0);
            for _ in 0..8 {
                if Instant::now() < self.next_tick || self.progress.outcome.is_some() {
                    break;
                }
                let mut inputs = vec![PlayerInput::default(); game.player_count()];
                for peer in self.peers.values_mut() {
                    if let Some(index) = peer.index {
                        inputs[index] = peer.controls.take();
                    }
                }
                let started = Instant::now();
                let events = game.step(&inputs);
                {
                    let mut metrics = self.metrics.lock().unwrap();
                    metrics.ticks += 1;
                    Metrics::sample(&mut metrics.tick_us, started);
                }
                self.progress.record_tick(game.world().winner(), &events);
                self.events
                    .extend(events.into_iter().take(2048 - self.events.len().min(2048)));
                self.next_tick += tick;
            }
            if Instant::now().saturating_duration_since(self.next_tick) > Duration::from_millis(250)
            {
                self.metrics.lock().unwrap().capped_catchups += 1;
                self.next_tick = Instant::now();
            }
            if Instant::now() >= self.next_snapshot {
                let started = Instant::now();
                let sent = started.duration_since(self.epoch).as_micros() as u64;
                game.prepare_snapshot();
                let mut shared = Frame::capture(game.for_player(0).unwrap());
                shared.weapons = game.weapon_effects();
                {
                    let mut metrics = self.metrics.lock().unwrap();
                    metrics.peak_bodies = metrics.peak_bodies.max(shared.bodies.len());
                }
                // Every trusted peer receives the same bodies: capture them once.
                let records = replication::Records::capture(&std::mem::take(&mut shared.bodies));
                for peer in self.peers.values_mut() {
                    let Some(index) = peer.index else {
                        continue;
                    };
                    if peer.connection.pending() > 0 {
                        let mut metrics = self.metrics.lock().unwrap();
                        metrics.skipped_snapshots += 1;
                        metrics.peak_queued_bytes =
                            metrics.peak_queued_bytes.max(peer.connection.pending());
                        continue;
                    }
                    let mut frame = if index == 0 {
                        shared.clone()
                    } else {
                        Frame::for_player(game.for_player(index).unwrap(), &shared)
                    };
                    replication::strip_diagnostics(&mut frame);
                    frame.stats.shots = game.shots(index);
                    // Trusted clients filter presentation events by their own sight.
                    let events = self.events.clone();
                    let mut progress = self.progress.clone();
                    progress.config.team = frame.team;
                    let delta = peer.encoder.encode(frame, &records);
                    if peer
                        .connection
                        .send(&ServerMessage::Snapshot {
                            delta: Box::new(delta),
                            events,
                            progress,
                            sent,
                        })
                        .is_err()
                    {
                        // A failed delta cannot be silently skipped: its base was advanced.
                        peer.last_seen = Instant::now() - Duration::from_secs(20);
                    }
                }
                Metrics::sample(&mut self.metrics.lock().unwrap().snapshot_us, started);
                self.events.clear();
                self.next_snapshot += SNAPSHOT_INTERVAL;
                if self.next_snapshot < Instant::now() {
                    self.next_snapshot = Instant::now();
                }
            }
        }
        let dead: Vec<_> = self
            .peers
            .iter_mut()
            .filter_map(|(&slot, p)| p.connection.flush().err().map(|_| slot))
            .collect();
        for slot in dead {
            self.remove(slot);
        }
        true
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    fn input(sequence: u32, fire: u32, special: u32) -> Input {
        Input {
            sequence,
            fire,
            special,
            deploy: 0,
            control: PlayerInput {
                tracks: [1.0; 2],
                ..Default::default()
            },
        }
    }
    #[test]
    fn vehicle_choice_survives_being_overtaken_and_repeats_do_not_redeploy() {
        let choose = |sequence, deploy| {
            let mut input = input(sequence, 0, 0);
            input.deploy = deploy;
            input.control.deploy = Some(sim::VehicleKind::Scout);
            input
        };
        let mut controls = ControlState::new();
        // The datagram after the choice overtakes the TCP copy carrying it.
        controls.receive_input(choose(2, 1));
        controls.receive_input(choose(1, 1));
        assert_eq!(controls.take().deploy, Some(sim::VehicleKind::Scout));
        // Later inputs repeat the choice without counting a new one.
        controls.receive_input(choose(3, 1));
        assert_eq!(controls.take().deploy, None);
        controls.receive_input(choose(4, 0));
        assert_eq!(controls.sequence, Some(3), "regressing counter is rejected");
        controls.receive_input(choose(5, 2));
        assert_eq!(controls.take().deploy, Some(sim::VehicleKind::Scout));
    }
    #[test]
    fn accepted_actions_survive_stalls_once_while_stale_movement_brakes() {
        let mut controls = ControlState::new();
        controls.receive_input(input(1, 1, 1));
        controls.last_input = Instant::now() - Duration::from_secs(1);
        let applied = controls.take();
        assert_eq!(applied.tracks, [0.0; 2]);
        assert!(applied.fire && applied.special);
        controls.receive_input(input(1, 1, 1));
        let duplicate = controls.take();
        assert!(!duplicate.fire && !duplicate.special);
        controls.receive_input(input(2, 1, 1));
        let held = controls.take();
        assert!(!held.fire && !held.special);
        assert_eq!(held.tracks, [1.0; 2]);
    }
    #[test]
    fn malformed_controls_old_sequences_and_regressing_counters_are_ignored() {
        let mut controls = ControlState::new();
        controls.receive_input(input(10, 1, 0));
        controls.take();
        controls.receive_input(input(9, 2, 1));
        assert!(!controls.take().fire);
        controls.receive_input(input(11, 0, 0));
        assert_eq!(controls.sequence, Some(10));
        let mut malformed = input(11, 2, 1);
        malformed.control.aim = sim::Vec2::splat(f32::NAN);
        controls.receive_input(malformed);
        assert_eq!(controls.sequence, Some(10));
        malformed = input(11, 2, 1);
        malformed.control.ammunition = sim::Ammunition::Plasma;
        controls.receive_input(malformed);
        assert_eq!(controls.sequence, Some(10));
        let mut forged = input(11, 1, 0);
        forged.control.fire = true;
        forged.control.special = true;
        controls.receive_input(forged);
        let applied = controls.take();
        assert!(!applied.fire && !applied.special);
    }
    #[test]
    fn sequence_and_action_counters_wrap_without_replaying_old_packets() {
        let mut controls = ControlState::new();
        controls.sequence = Some(u32::MAX);
        controls.fire = u32::MAX;
        controls.special = u32::MAX;
        controls.receive_input(input(0, 0, 0));
        let applied = controls.take();
        assert!(applied.fire && applied.special);
        controls.receive_input(input(u32::MAX, u32::MAX, u32::MAX));
        let old = controls.take();
        assert!(!old.fire && !old.special);
    }
}
