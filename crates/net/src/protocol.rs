use crate::replication::Delta;
use serde::{Deserialize, Serialize};
use sim::{
    level::Team,
    matchplay::{MatchConfig, MatchProgress},
    multiplayer::PlayerInput,
    replica::Map,
};
pub const VERSION: u32 = 10;
pub const PORT: u16 = 45771;
pub const DISCOVERY_PORT: u16 = 45770;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub slot: u8,
    pub name: String,
    pub team: Team,
    pub ready: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lobby {
    pub revision: u64,
    pub name: String,
    pub map_name: String,
    pub map_size: [f32; 2],
    pub map_objects: usize,
    pub config: MatchConfig,
    pub players: Vec<Player>,
    pub playing: bool,
}
impl Lobby {
    pub fn can_start(&self) -> bool {
        !self.playing
            && self.players.len() >= 2
            && self.players.len() <= 6
            && self.players.iter().all(|p| p.ready)
            && self.players.iter().any(|p| p.team == Team::Blue)
            && self.players.iter().any(|p| p.team == Team::Red)
    }
}
/// Latest controls. One-shot actions are cumulative counters, so any newer
/// input carries every earlier action and a lost or overtaken copy is harmless.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Input {
    pub sequence: u32,
    pub fire: u32,
    pub special: u32,
    /// Counts vehicle choices; `control.deploy` repeats the latest one.
    pub deploy: u32,
    pub control: PlayerInput,
}
/// Where to send unreliable input copies, and the session secret proving the
/// sender owns its slot.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InputChannel {
    pub port: u16,
    pub token: u64,
}
/// A UDP copy of the latest input. Each one supersedes earlier ones, so loss
/// or reordering never holds controls behind a TCP retransmission.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InputDatagram {
    pub slot: u8,
    pub token: u64,
    pub input: Input,
}
#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    Hello { version: u32, name: String },
    Team(Team),
    Ready { revision: u64, ready: bool },
    Input(Input),
    Ping,
    Leave,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        slot: u8,
        input: Option<InputChannel>,
    },
    Lobby(Lobby),
    Started {
        map: Map,
        progress: MatchProgress,
    },
    Snapshot {
        delta: Box<Delta>,
        events: Vec<sim::Event>,
        progress: MatchProgress,
        /// Host clock at capture, in microseconds. Unlike the tick count it
        /// advances through the countdown and host catch-up limits, so clients
        /// can measure network jitter from it.
        sent: u64,
    },
    Error(String),
    Pong,
}
pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 32 && !name.chars().any(char::is_control)
}
