//! Stateless LAN discovery. TCP carries gameplay; broadcasts only locate hosts.
use crate::{
    codec,
    protocol::{self, Lobby},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
const QUERY: &[u8] = b"TANK-LAN1?";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Advertisement {
    pub version: u32,
    pub name: String,
    pub map: String,
    pub players: u8,
    pub port: u16,
    pub playing: bool,
}
pub(crate) fn respond(socket: &UdpSocket, port: u16, lobby: &Lobby) {
    let mut data = [0u8; 512];
    for _ in 0..16 {
        let Ok((n, address)) = socket.recv_from(&mut data) else {
            break;
        };
        if &data[..n] != QUERY {
            continue;
        }
        let ad = Advertisement {
            version: protocol::VERSION,
            name: lobby.name.clone(),
            map: lobby.map_name.clone(),
            players: lobby.players.len() as u8,
            port,
            playing: lobby.playing,
        };
        if let Ok(bytes) = codec::encode(&ad) {
            let _ = socket.send_to(&bytes, address);
        }
    }
}
pub struct Discovery {
    socket: UdpSocket,
    found: BTreeMap<SocketAddr, (Advertisement, Instant)>,
    last: Instant,
    port: u16,
}
impl Discovery {
    pub fn new() -> io::Result<Self> {
        Self::on_port(protocol::DISCOVERY_PORT)
    }
    pub fn on_port(port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            found: BTreeMap::new(),
            last: Instant::now() - Duration::from_secs(2),
            port,
        })
    }
    pub fn poll(&mut self) -> Vec<(SocketAddr, Advertisement)> {
        if self.last.elapsed() >= Duration::from_secs(1) {
            for ip in [Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST] {
                let _ = self.socket.send_to(QUERY, (ip, self.port));
            }
            self.last = Instant::now();
        }
        let mut data = [0u8; 2048];
        for _ in 0..32 {
            let Ok((n, from)) = self.socket.recv_from(&mut data) else {
                break;
            };
            if let Ok(Some((ad, used))) = codec::decode::<Advertisement>(&data[..n])
                && used == n
                && ad.version == protocol::VERSION
                && ad.name.len() <= 64
                && ad.map.len() <= 128
                && ad.players <= 6
            {
                let address = SocketAddr::new(from.ip(), ad.port);
                self.found.insert(address, (ad, Instant::now()));
            }
        }
        self.found
            .retain(|_, (_, seen)| seen.elapsed() < Duration::from_secs(4));
        self.found
            .iter()
            .map(|(a, (ad, _))| (*a, ad.clone()))
            .collect()
    }
}
pub fn parse_address(input: &str) -> Result<SocketAddr, String> {
    let input = input.trim();
    input
        .parse::<SocketAddr>()
        .or_else(|_| {
            input
                .parse::<IpAddr>()
                .map(|ip| SocketAddr::new(ip, protocol::PORT))
        })
        .map_err(|_| "Enter an IP address, optionally followed by :port.".into())
}
