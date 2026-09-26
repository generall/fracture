//! Local lobby and match presentation. The simulation owns match rules/results.
mod hud;
mod menus;
use crate::{game_storage, lab::Lab};
use macroquad::prelude::*;
use sim::{level::Level, matchplay::MatchConfig};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Screen {
    Home,
    Lobby,
    Workshop,
    Match,
    Sandbox,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Lan,
}
pub enum Action {
    EnterLobby,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    EnterLan,
    CreateMap,
    EditMap,
    Start,
    Resume,
    Leave,
}
pub struct Game {
    #[cfg(not(target_arch = "wasm32"))]
    pub lan: Option<crate::lan::Lan>,
    pub screen: Screen,
    pub map: Level,
    pub config: MatchConfig,
    pub countdown: f32,
    pub menu: bool,
    pub help: bool,
    pub status: String,
    custom: Option<Level>,
    draft: Option<Level>,
    source: usize,
    terrain: Vec<sim::TerrainPatch>,
}
impl Game {
    pub fn network_host(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.lan.as_ref().is_some_and(|lan| lan.is_host())
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    pub fn new(sandbox: bool) -> Self {
        let custom =
            game_storage::restore().and_then(|j| j.map(|j| Level::from_json(&j)).transpose());
        let draft = crate::level_storage::restore()
            .and_then(|j| j.map(|j| Level::from_json(&j)).transpose());
        let status = custom
            .as_ref()
            .err()
            .or_else(|| draft.as_ref().err())
            .cloned()
            .unwrap_or_default();
        let mut game = Self {
            #[cfg(not(target_arch = "wasm32"))]
            lan: None,
            screen: if sandbox {
                Screen::Sandbox
            } else {
                Screen::Home
            },
            map: Level::demo(),
            config: MatchConfig::default(),
            countdown: 0.0,
            menu: false,
            help: false,
            status,
            custom: custom.ok().flatten(),
            draft: draft.ok().flatten(),
            source: 0,
            terrain: vec![],
        };
        game.refresh_preview();
        game
    }
    fn refresh_preview(&mut self) {
        self.terrain = self.map.terrain_mask.patches();
        self.terrain
            .extend(self.map.items.iter().filter_map(|item| match item {
                sim::level::LevelItem::Terrain { terrain, min, max } => Some(sim::TerrainPatch {
                    kind: *terrain,
                    min: (*min).into(),
                    max: (*max).into(),
                }),
                sim::level::LevelItem::Bridge { min, max, .. } => Some(sim::TerrainPatch {
                    kind: sim::TerrainKind::Bridge,
                    min: (*min).into(),
                    max: (*max).into(),
                }),
                _ => None,
            }));
    }
    pub(crate) fn select_map(&mut self, source: usize) {
        let map = match source {
            0 => Some(Level::demo()),
            1 => Some(Level::river_valley()),
            2 => self.draft.clone(),
            _ => self.custom.clone(),
        };
        if let Some(map) = map {
            self.map = map;
            self.source = source;
            self.status.clear();
            self.refresh_preview();
        }
    }
    pub fn enter_lobby(&mut self) {
        match crate::level_storage::restore()
            .and_then(|j| j.map(|j| Level::from_json(&j)).transpose())
        {
            Ok(draft) => self.draft = draft,
            Err(e) => self.status = e,
        }
        self.screen = Screen::Lobby;
    }
    pub fn accept_map(&mut self, map: Level) {
        match map.to_json().and_then(|json| game_storage::persist(&json)) {
            Ok(()) => self.status = "Your map is saved on this device.".into(),
            Err(e) => self.status = e,
        }
        self.map = map;
        self.custom = Some(self.map.clone());
        self.source = 3;
        self.refresh_preview();
        self.screen = Screen::Lobby;
    }
    pub fn start(&mut self, lab: &mut Lab) {
        match lab.start_match(&self.map, self.config) {
            Ok(()) => {
                self.screen = Screen::Match;
                self.countdown = 3.0;
                self.menu = false;
                self.help = false;
                self.status.clear();
            }
            Err(e) => {
                self.status = e;
                self.screen = Screen::Lobby;
            }
        }
    }
    pub fn return_to_lobby(&mut self, lab: &mut Lab) {
        self.screen = Screen::Lobby;
        self.menu = false;
        self.countdown = 0.0;
        lab.pending.clear();
        lab.firing = false;
        lab.accumulator = 0.0;
        lab.paused = false;
    }
    pub fn input_allowed(&self, lab: &Lab) -> bool {
        self.screen == Screen::Match
            && self.countdown <= 0.0
            && !self.menu
            && lab
                .match_progress
                .as_ref()
                .is_some_and(|p| p.outcome.is_none())
    }
    pub fn advance(&mut self, dt: f32, lab: &mut Lab) {
        if self.screen != Screen::Match {
            return;
        }
        if !self.menu {
            self.countdown = (self.countdown - dt).max(0.0);
        }
        lab.paused = !self.input_allowed(lab);
        if lab.paused {
            lab.pending.clear();
            lab.firing = false;
            lab.accumulator = 0.0;
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod diagnostics {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    static READY: AtomicU32 = AtomicU32::new(0);
    static SCREEN: AtomicU32 = AtomicU32::new(0);
    static PHASE: AtomicU32 = AtomicU32::new(0);
    static TICKS: AtomicU32 = AtomicU32::new(0);
    pub fn publish(game: &Game, lab: &Lab) {
        READY.store(1, Ordering::Relaxed);
        SCREEN.store(game.screen as u32, Ordering::Relaxed);
        PHASE.store(
            if game.screen != Screen::Match {
                0
            } else if lab
                .match_progress
                .as_ref()
                .is_some_and(|p| p.outcome.is_some())
            {
                4
            } else if game.menu {
                3
            } else if game.countdown > 0.0 {
                1
            } else {
                2
            },
            Ordering::Relaxed,
        );
        TICKS.store(
            lab.match_progress
                .as_ref()
                .map_or(0, |p| p.elapsed_ticks as u32),
            Ordering::Relaxed,
        );
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn game_ready() -> u32 {
        READY.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn game_screen() -> u32 {
        SCREEN.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn game_phase() -> u32 {
        PHASE.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn game_match_ticks() -> u32 {
        TICKS.load(Ordering::Relaxed)
    }
}
#[cfg(target_arch = "wasm32")]
pub use diagnostics::publish;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn countdown_pause_results_and_rematch_gate_gameplay() {
        let mut game = Game::new(false);
        game.map = Level::default();
        let mut lab = Lab::new();
        game.start(&mut lab);
        assert!(game.screen == Screen::Match);
        assert!(!game.input_allowed(&lab));
        game.advance(1.0, &mut lab);
        assert!(lab.paused);
        game.menu = true;
        game.advance(10.0, &mut lab);
        assert_eq!(game.countdown, 2.0);
        game.menu = false;
        game.advance(2.0, &mut lab);
        assert!(game.input_allowed(&lab));
        assert!(!lab.paused);
        lab.pending.push(sim::Command::ActivateSpecial);
        game.menu = true;
        game.advance(0.1, &mut lab);
        assert!(lab.pending.is_empty());
        assert!(lab.paused);
        game.menu = false;
        lab.match_progress
            .as_mut()
            .unwrap()
            .record_tick(Some(sim::level::Team::Blue), &[]);
        game.advance(0.1, &mut lab);
        assert!(!game.input_allowed(&lab));
        game.start(&mut lab);
        assert_eq!(game.countdown, 3.0);
        assert_eq!(lab.match_progress.as_ref().unwrap().elapsed_ticks, 0);
        assert_eq!(lab.match_progress.as_ref().unwrap().outcome, None);
        game.return_to_lobby(&mut lab);
        assert!(game.screen == Screen::Lobby);
        assert!(!game.input_allowed(&lab));
    }
    #[test]
    fn invalid_map_keeps_the_player_in_the_lobby() {
        let mut game = Game::new(false);
        game.map = Level::default();
        game.map
            .items
            .retain(|item| !matches!(item, sim::level::LevelItem::Flag { .. }));
        let mut lab = Lab::new();
        game.start(&mut lab);
        assert!(game.screen == Screen::Lobby);
        assert!(game.status.contains("flag"));
        assert!(lab.match_progress.is_none());
    }
}
