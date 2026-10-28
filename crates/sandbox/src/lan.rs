//! Desktop LAN screens and snapshot presentation. No simulation runs here.
use crate::ui::PINK;
use crate::{
    game::{Action, Game, Screen},
    lab::Lab,
    ui::*,
};
use macroquad::prelude::*;
use sim::{
    level::Team,
    matchplay::MatchProgress,
    multiplayer::PlayerInput,
    replica::{Frame, Map},
};
use std::{
    net::SocketAddr,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
use tank_net::{
    discovery::{self, Discovery},
    playout::Playout,
    protocol::{ClientMessage, Input, Lobby, ServerMessage},
    replication::{self, Decoder},
    session::{Client, Host, HostCommand},
};

pub struct Lan {
    name: String,
    address: String,
    focus: usize,
    status: String,
    host: Option<Host>,
    client: Option<Client>,
    connecting: Option<Receiver<Result<Client, String>>>,
    discovery: Option<Discovery>,
    lobby: Option<Lobby>,
    slot: Option<u8>,
    map: Option<Map>,
    decoder: Decoder,
    /// Last presented frame: the interpolation start for the next one.
    previous: Option<Frame>,
    playout: Playout<(Frame, Vec<sim::Event>, MatchProgress)>,
    epoch: Instant,
    sequence: u32,
    fire: u32,
    special: u32,
    /// Vehicle choices made, and the latest one, repeated in every input.
    deploys: u32,
    deploy: Option<sim::VehicleKind>,
    last_input: Instant,
}
impl Default for Lan {
    fn default() -> Self {
        Self {
            name: "Commander".into(),
            address: "127.0.0.1".into(),
            focus: 0,
            status: String::new(),
            host: None,
            client: None,
            connecting: None,
            discovery: Discovery::new().ok(),
            lobby: None,
            slot: None,
            map: None,
            decoder: Decoder::default(),
            previous: None,
            playout: Playout::default(),
            epoch: Instant::now(),
            sequence: 0,
            fire: 0,
            special: 0,
            deploys: 0,
            deploy: None,
            last_input: Instant::now(),
        }
    }
}
impl Lan {
    pub fn is_host(&self) -> bool {
        self.host.is_some()
    }
    pub fn alpha(&self) -> f32 {
        self.playout.alpha()
    }
    fn reset_snapshots(&mut self) {
        self.previous = None;
        self.decoder = Decoder::default();
        self.playout = Playout::default();
    }
    fn send(&mut self, message: ClientMessage) {
        if let Some(client) = &mut self.client
            && let Err(e) = client.send(message)
        {
            self.status = format!("Connection error: {e}");
        }
    }
    fn command(&mut self, command: HostCommand) {
        if let Some(host) = &self.host
            && let Err(e) = host.command(command)
        {
            self.status = e;
        }
    }
    fn configure(&mut self, game: &Game) {
        self.command(HostCommand::Configure {
            level: Box::new(game.map.clone()),
            config: game.config,
        });
    }
    fn join(&mut self, address: SocketAddr) {
        let name = self.name.clone();
        let (tx, rx) = mpsc::channel();
        self.connecting = Some(rx);
        self.status = format!("Connecting to {address}…");
        std::thread::spawn(move || {
            let _ = tx.send(Client::connect(address, &name).map_err(|e| e.to_string()));
        });
    }
    fn disconnect(&mut self) {
        // Do not turn earlier driving keys into name/address text on return.
        while get_char_pressed().is_some() {}
        if let Some(client) = &mut self.client {
            let _ = client.send(ClientMessage::Leave);
        }
        self.client = None;
        self.host = None;
        self.lobby = None;
        self.slot = None;
        self.map = None;
        self.reset_snapshots();
        self.connecting = None;
        self.sequence = 0;
        self.fire = 0;
        self.special = 0;
        self.deploys = 0;
        self.deploy = None;
    }
    fn poll(&mut self, game: &mut Game, lab: &mut Lab) {
        if let Some(rx) = &self.connecting
            && let Ok(result) = rx.try_recv()
        {
            self.connecting = None;
            match result {
                Ok(client) => {
                    self.client = Some(client);
                    self.status.clear();
                }
                Err(e) => self.status = format!("Could not join: {e}"),
            }
        }
        let messages = match self.client.as_mut().map(Client::poll) {
            Some(Ok(messages)) => messages,
            Some(Err(e)) => {
                self.status = format!("Disconnected: {e}");
                self.disconnect();
                game.screen = Screen::Lan;
                game.menu = false;
                lab.pending.clear();
                return;
            }
            None => return,
        };
        for message in messages {
            match message {
                ServerMessage::Welcome { slot, .. } => {
                    self.slot = Some(slot);
                    self.status.clear();
                }
                ServerMessage::Lobby(lobby) => {
                    if !lobby.playing && game.screen == Screen::Match {
                        game.screen = Screen::Lan;
                        game.menu = false;
                        self.reset_snapshots();
                    }
                    if lobby.playing && game.screen == Screen::Match {
                        self.announce_roster(&lobby, lab);
                    }
                    self.lobby = Some(lobby);
                }
                ServerMessage::Started { map, progress } => {
                    self.map = Some(map);
                    self.reset_snapshots();
                    // Joining a running match skips the opening countdown.
                    game.countdown = if progress.elapsed_ticks > 0 { 0.0 } else { 3.0 };
                    lab.match_progress = Some(progress);
                    game.menu = false;
                }
                ServerMessage::Snapshot {
                    delta,
                    events,
                    progress,
                    sent,
                } => match self.decoder.decode(*delta) {
                    Ok(frame) => {
                        let now = self.epoch.elapsed().as_secs_f64();
                        self.playout
                            .push(sent as f64 * 1e-6, now, (frame, events, progress));
                    }
                    Err(e) => {
                        self.status = format!("Invalid host update: {e}");
                        self.disconnect();
                        game.screen = Screen::Lan;
                        return;
                    }
                },
                ServerMessage::Error(e) => {
                    self.status = e;
                    if self.slot.is_none() {
                        self.disconnect();
                    }
                }
                ServerMessage::Pong => {}
            }
        }
        let due = self.playout.advance(self.epoch.elapsed().as_secs_f64());
        let mut events = vec![];
        for (frame, frame_events, progress) in due {
            if !self.present(game, lab, frame, progress) {
                return;
            }
            events.extend(frame_events);
        }
        if !events.is_empty() {
            // Trusted clients receive every event and keep only what they can see.
            let vision = lab.world.player_vision(0.0);
            let team = lab.world.team();
            let events = replication::visible_events(&events, &vision, lab.world.player_tank_id());
            for event in &events {
                if let Some(cue) = crate::audio::Cue::event(event, team) {
                    cue.enqueue(&mut lab.audio_cues, lab.world.launcher_position());
                }
            }
            lab.present_events(&events);
        }
    }
    /// Names players who joined or left the running match since the last roster.
    fn announce_roster(&self, lobby: &Lobby, lab: &mut Lab) {
        let Some(old) = &self.lobby else {
            return;
        };
        let joined = lobby
            .players
            .iter()
            .filter(|p| Some(p.slot) != self.slot)
            .find(|p| {
                !old.players
                    .iter()
                    .any(|o| o.slot == p.slot && o.name == p.name)
            })
            .map(|p| format!("{} joined {}", p.name, p.team.label()));
        let left = old
            .players
            .iter()
            .find(|o| {
                !lobby
                    .players
                    .iter()
                    .any(|p| p.slot == o.slot && p.name == o.name)
            })
            .map(|o| format!("{} left the match", o.name));
        if let Some(message) = joined.or(left) {
            lab.notice = Some((message, 3.0));
        }
    }
    /// Shows one buffered snapshot. Returns false if the session was abandoned.
    fn present(
        &mut self,
        game: &mut Game,
        lab: &mut Lab,
        mut frame: Frame,
        progress: MatchProgress,
    ) -> bool {
        let first = self.previous.is_none();
        let charges = lab.world.special_charges();
        let reloading = lab.world.reload_remaining() > 0.0;
        if let Some(previous) = &self.previous {
            frame.interpolate_from(previous);
        }
        if first {
            let Some(map) = self.map.take() else {
                self.status = "Host omitted match map.".into();
                self.disconnect();
                game.screen = Screen::Lan;
                return false;
            };
            lab.world = sim::World::from_replica(map, frame.clone());
            lab.clear_session();
            lab.inspector = crate::ui::Inspector::default();
            lab.fog_enabled = true;
            lab.overlay = false;
            lab.slow = false;
            lab.paused = false;
            game.screen = Screen::Match;
        } else {
            lab.track_marks.seed(&lab.world);
            lab.world.apply_replica(frame.clone());
            lab.track_marks.update(&lab.world);
        }
        lab.match_progress = Some(progress);
        lab.vision = lab.world.player_vision(0.0);
        if frame.stats.ticks > 0 {
            game.countdown = 0.0;
        }
        if !first && frame.charges > charges {
            lab.audio_cues.push(crate::audio::Cue::local(
                crate::audio::Clip::Pickup,
                &lab.world,
            ));
        }
        if !first && reloading && frame.reload <= 0.0 && frame.alive {
            lab.audio_cues.push(crate::audio::Cue::local(
                crate::audio::Clip::Reload,
                &lab.world,
            ));
        }
        self.previous = Some(frame);
        true
    }
    fn input(&mut self, lab: &mut Lab, enabled: bool, tracks: [f32; 2]) {
        if !lab.world.is_replica() || self.client.is_none() {
            return;
        }
        if enabled
            && lab.firing
            && lab.world.player_vehicle() == sim::VehicleKind::Scout
            && lab.world.reload_remaining() <= 0.0
        {
            lab.fire();
        }
        let fire = enabled
            && lab
                .pending
                .iter()
                .any(|c| matches!(c, sim::Command::Fire(_)));
        let special = enabled
            && lab
                .pending
                .iter()
                .any(|c| matches!(c, sim::Command::ActivateSpecial));
        if fire {
            self.fire = self.fire.wrapping_add(1);
        }
        if special {
            self.special = self.special.wrapping_add(1);
        }
        let deploy = lab.pending.iter().find_map(|c| {
            if let sim::Command::Deploy(kind) = c {
                Some(*kind)
            } else {
                None
            }
        });
        lab.pending.clear();
        if deploy.is_some() {
            self.deploys = self.deploys.wrapping_add(1);
            self.deploy = deploy;
        }
        if deploy.is_none()
            && !fire
            && !special
            && self.last_input.elapsed() < Duration::from_secs_f32(1.0 / 60.0)
        {
            return;
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.last_input = Instant::now();
        let input = Input {
            sequence: self.sequence,
            fire: self.fire,
            special: self.special,
            deploy: self.deploys,
            control: PlayerInput {
                deploy: self.deploy,
                tracks: if enabled { tracks } else { [0.0; 2] },
                aim: sim::Vec2::new(lab.aim.x, lab.aim.y),
                ammunition: lab.inspector.ammunition,
                fire: false,
                special: false,
            },
        };
        if let Some(client) = &mut self.client
            && let Err(e) = client.send_input(input)
        {
            self.status = format!("Connection error: {e}");
        }
    }
    pub fn draw(&mut self, game: &mut Game, size: Vec2, mouse: Vec2) -> Option<Action> {
        let compact = size.x < 1400.0 || size.y < 860.0;
        text("FRACTURE / LOCAL NETWORK", 64.0, 68.0, 30.0, INK);
        text(
            "UP TO SIX PLAYERS / CAPTURE THE FLAG",
            64.0,
            105.0,
            17.0,
            MUTED,
        );
        if button(
            Rect::new(size.x - 260.0, 42.0, 196.0, 40.0),
            if self.client.is_some() {
                "LEAVE SESSION"
            } else {
                "BACK"
            },
            mouse,
            false,
        ) || crate::controller_ui::back()
        {
            self.disconnect();
            game.screen = Screen::Home;
            return None;
        }
        draw_line(64.0, 135.0, size.x - 64.0, 135.0, 1.0, RULE);
        if let Some(lobby) = self.lobby.clone() {
            text(&lobby.name, 64.0, 180.0, 23.0, CYAN);
            text(
                &format!(
                    "{} / {:.0} x {:.0} m / {} objects",
                    lobby.map_name, lobby.map_size[0], lobby.map_size[1], lobby.map_objects
                ),
                64.0,
                214.0,
                18.0,
                INK,
            );
            text(
                &format!(
                    "TIME / {}    AIRDROPS / {}",
                    lobby
                        .config
                        .time_limit_seconds
                        .map_or("UNLIMITED".into(), |s| format!("{} MIN", s / 60)),
                    if lobby.config.airdrops { "ON" } else { "OFF" }
                ),
                64.0,
                243.0,
                17.0,
                MUTED,
            );
            for (i, p) in lobby.players.iter().enumerate() {
                let y = if compact {
                    270.0 + i as f32 * 35.0
                } else {
                    290.0 + i as f32 * 57.0
                };
                draw_rectangle_lines(
                    64.0,
                    y - 26.0,
                    size.x - 128.0,
                    if compact { 33.0 } else { 49.0 },
                    1.0,
                    RULE,
                );
                text(
                    &format!(
                        "{}{}{}",
                        p.name,
                        if p.slot == 0 { " / HOST" } else { "" },
                        if Some(p.slot) == self.slot {
                            " / YOU"
                        } else {
                            ""
                        }
                    ),
                    82.0,
                    y + 5.0,
                    20.0,
                    INK,
                );
                text(
                    p.team.label(),
                    size.x * 0.57,
                    y + 5.0,
                    20.0,
                    if p.team == Team::Blue { CYAN } else { PINK },
                );
                text(
                    if p.ready { "READY" } else { "NOT READY" },
                    size.x - 262.0,
                    y + 5.0,
                    18.0,
                    if p.ready { CYAN } else { MUTED },
                );
            }
            if lobby.playing {
                text(
                    "Match in progress. Joining…",
                    64.0,
                    if compact { size.y - 230.0 } else { 670.0 },
                    18.0,
                    MUTED,
                );
            } else if let Some(player) = lobby.players.iter().find(|p| Some(p.slot) == self.slot) {
                if button(
                    Rect::new(
                        64.0,
                        if compact { size.y - 250.0 } else { 650.0 },
                        260.0,
                        42.0,
                    ),
                    "CHANGE TEAM",
                    mouse,
                    false,
                ) {
                    self.send(ClientMessage::Team(player.team.other()));
                }
                if button(
                    Rect::new(
                        340.0,
                        if compact { size.y - 250.0 } else { 650.0 },
                        260.0,
                        42.0,
                    ),
                    if player.ready { "UNREADY" } else { "READY" },
                    mouse,
                    player.ready,
                ) {
                    self.send(ClientMessage::Ready {
                        revision: lobby.revision,
                        ready: !player.ready,
                    });
                }
            }
            if self.is_host() && !lobby.playing {
                for (index, label) in game.map_choices().into_iter().enumerate() {
                    if button(
                        Rect::new(
                            64.0 + index as f32 * 264.0,
                            if compact { size.y - 194.0 } else { 710.0 },
                            250.0,
                            38.0,
                        ),
                        label,
                        mouse,
                        false,
                    ) {
                        game.select_map(index);
                        self.configure(game);
                    }
                }
                if button(
                    Rect::new(
                        size.x - 320.0,
                        if compact { size.y - 194.0 } else { 710.0 },
                        256.0,
                        38.0,
                    ),
                    "MATCH TIME",
                    mouse,
                    false,
                ) {
                    game.config.time_limit_seconds = match game.config.time_limit_seconds {
                        Some(600) => None,
                        None => Some(300),
                        _ => Some(600),
                    };
                    self.configure(game);
                }
                if button(
                    Rect::new(
                        64.0,
                        if compact { size.y - 144.0 } else { 760.0 },
                        250.0,
                        38.0,
                    ),
                    "IMPORT MAP",
                    mouse,
                    false,
                ) && let Err(e) = crate::level_storage::pick_import()
                {
                    self.status = e;
                }
                if button(
                    Rect::new(
                        328.0,
                        if compact { size.y - 144.0 } else { 760.0 },
                        250.0,
                        38.0,
                    ),
                    if game.config.airdrops {
                        "AIRDROPS / ON"
                    } else {
                        "AIRDROPS / OFF"
                    },
                    mouse,
                    game.config.airdrops,
                ) {
                    game.config.airdrops = !game.config.airdrops;
                    self.configure(game);
                }
                if let Some(import) = crate::level_storage::take_import() {
                    match import.and_then(|s| sim::level::Level::from_json(&s)) {
                        Ok(map) => {
                            game.accept_map(map);
                            game.screen = Screen::Lan;
                            self.configure(game);
                        }
                        Err(e) => self.status = e,
                    }
                }
                if button(
                    Rect::new(
                        size.x - 320.0,
                        if compact { size.y - 144.0 } else { 760.0 },
                        256.0,
                        46.0,
                    ),
                    "START MATCH  >",
                    mouse,
                    lobby.can_start(),
                ) {
                    self.command(HostCommand::Start);
                }
            } else if !lobby.playing {
                text(
                    "The host chooses the map and starts when everyone is ready.",
                    64.0,
                    if compact { size.y - 175.0 } else { 740.0 },
                    18.0,
                    MUTED,
                );
            }
        } else {
            self.field(Rect::new(64.0, 210.0, 440.0, 46.0), "YOUR NAME", 0, mouse);
            if self.client.is_none()
                && self.connecting.is_none()
                && button(
                    Rect::new(64.0, 290.0, 440.0, 52.0),
                    "HOST A LOBBY  >",
                    mouse,
                    true,
                )
            {
                match Host::start(game.map.clone(), game.config, &self.name) {
                    Ok((host, client)) => {
                        self.host = Some(host);
                        self.client = Some(client);
                        self.status = "Creating lobby…".into();
                    }
                    Err(e) => self.status = e,
                }
            }
            self.field(
                Rect::new(64.0, 418.0, 440.0, 46.0),
                "HOST IP ADDRESS",
                1,
                mouse,
            );
            if self.client.is_none()
                && self.connecting.is_none()
                && button(
                    Rect::new(64.0, 485.0, 440.0, 52.0),
                    "JOIN BY IP  >",
                    mouse,
                    false,
                )
            {
                match discovery::parse_address(&self.address) {
                    Ok(address) => self.join(address),
                    Err(e) => self.status = e,
                }
            }
            text("LOBBIES ON YOUR NETWORK", 590.0, 190.0, 20.0, CYAN);
            let found = self
                .discovery
                .as_mut()
                .map(Discovery::poll)
                .unwrap_or_default();
            if found.is_empty() {
                wrapped(
                    "Looking for hosts… You can also connect directly using the host computer's IP address.",
                    590.0,
                    240.0,
                    size.x - 660.0,
                    18.0,
                    MUTED,
                );
            }
            for (i, (address, ad)) in found.into_iter().take(6).enumerate() {
                let y = 230.0 + i as f32 * 76.0;
                let full = ad.players >= 6;
                let label = format!(
                    "{}  /  {}/6{}",
                    ad.name,
                    ad.players,
                    match (ad.playing, full) {
                        (_, true) => " / FULL",
                        (true, false) => " / IN MATCH / JOIN",
                        (false, false) => "",
                    }
                );
                if button(
                    Rect::new(590.0, y, size.x - 654.0, 44.0),
                    &label,
                    mouse,
                    false,
                ) && !full
                    && self.client.is_none()
                    && self.connecting.is_none()
                {
                    self.join(address);
                }
                text(
                    &format!("{} / {}", address, ad.map),
                    602.0,
                    y + 63.0,
                    15.0,
                    MUTED,
                );
            }
            if self.client.is_none()
                && self.connecting.is_none()
                && !crate::controller_ui::keyboard_open()
            {
                let value = if self.focus == 0 {
                    &mut self.name
                } else {
                    &mut self.address
                };
                if is_key_pressed(KeyCode::Backspace) {
                    value.pop();
                }
                while let Some(c) = get_char_pressed() {
                    if !c.is_control()
                        && value.len() + c.len_utf8() <= if self.focus == 0 { 32 } else { 64 }
                    {
                        value.push(c);
                    }
                }
            }
        }
        if !self.status.is_empty() {
            wrapped(
                &self.status,
                64.0,
                size.y - 49.0,
                size.x - 128.0,
                17.0,
                PINK,
            );
        }
        None
    }
    fn field(&mut self, rect: Rect, label: &str, index: usize, mouse: Vec2) {
        text(label, rect.x, rect.y - 15.0, 17.0, MUTED);
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.0,
            if self.focus == index { CYAN } else { RULE },
        );
        field_text(
            if index == 0 {
                &self.name
            } else {
                &self.address
            },
            rect,
            29.0,
            20.0,
            self.focus == index && self.client.is_none() && self.connecting.is_none(),
        );
        let editable = self.client.is_none() && self.connecting.is_none();
        let (focused, activated) = if editable {
            crate::controller_ui::interact(rect, label, false)
        } else {
            (false, false)
        };
        if focused {
            draw_rectangle_lines(
                rect.x - 3.0,
                rect.y - 3.0,
                rect.w + 6.0,
                rect.h + 6.0,
                2.0,
                CYAN,
            );
        }
        let pointer_clicked = editable
            && crate::controller_ui::pointer_allowed()
            && is_mouse_button_pressed(MouseButton::Left)
            && rect.contains(mouse);
        if pointer_clicked {
            self.focus = index;
        }
        if activated || (pointer_clicked && crate::deck_mode()) {
            self.focus = index;
            crate::controller_ui::edit_text(
                index,
                if index == 0 {
                    &self.name
                } else {
                    &self.address
                },
                if index == 0 { 32 } else { 64 },
            );
        }
        if let Some(value) = crate::controller_ui::take_text(index) {
            if index == 0 {
                self.name = value;
            } else {
                self.address = value;
            }
        }
    }
}
impl Game {
    pub fn poll_lan(&mut self, lab: &mut Lab, _dt: f32) {
        if let Some(mut lan) = self.lan.take() {
            lan.poll(self, lab);
            self.lan = Some(lan);
        }
    }
    pub fn lan_input(&mut self, lab: &mut Lab, enabled: bool, tracks: [f32; 2]) {
        if let Some(lan) = &mut self.lan {
            lan.input(lab, enabled, tracks);
        }
    }
    pub fn lan_action(&mut self, action: &Action, lab: &mut Lab) -> bool {
        if self.screen != Screen::Match || !lab.world.is_replica() {
            return false;
        }
        let Some(lan) = &mut self.lan else {
            return false;
        };
        match action {
            Action::Start => {
                if lan.is_host() {
                    lan.command(HostCommand::ReturnToLobby);
                } else {
                    lab.notice = Some(("The host starts the next match.".into(), 3.0));
                }
            }
            Action::Leave => {
                lan.disconnect();
                self.screen = Screen::Lan;
                self.menu = false;
            }
            Action::Resume => self.menu = false,
            Action::Exit => {
                lan.disconnect();
                self.exit_requested = true;
            }
            _ => return false,
        }
        lab.pending.clear();
        lab.firing = false;
        true
    }
}
