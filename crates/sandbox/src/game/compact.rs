//! Handheld layouts keep text and controls large instead of shrinking a desktop.
use super::Action;
use super::*;
use crate::{controller_ui::prompt, ui::*};
use sim::level::EnemyActivity;

pub fn needed(size: Vec2) -> bool {
    size.x < 1400.0 || size.y < 860.0
}
impl Game {
    pub(super) fn compact_menu(&mut self, size: Vec2, mouse: Vec2) -> Option<Action> {
        if self.screen == Screen::Home {
            text("F R A C T U R E", 36.0, 160.0, 48.0, INK);
            text("TANK COMBAT", 40.0, 204.0, 24.0, CYAN);
            wrapped(
                "Break through cover. Capture the flag.",
                40.0,
                260.0,
                420.0,
                24.0,
                MUTED,
            );
            self.preview(Rect::new(520.0, 110.0, size.x - 556.0, size.y - 240.0));
            if button(
                Rect::new(36.0, 350.0, 430.0, 54.0),
                "LOCAL MATCH",
                mouse,
                true,
            ) || is_key_pressed(KeyCode::Enter)
            {
                return Some(Action::EnterLobby);
            }
            #[cfg(not(target_arch = "wasm32"))]
            if button(
                Rect::new(36.0, 424.0, 430.0, 54.0),
                "LOCAL NETWORK",
                mouse,
                false,
            ) {
                return Some(Action::EnterLan);
            }
            text(
                prompt(
                    "WASD DRIVE / Q E AIM / SPACE FIRE / G SPECIAL",
                    "LS DRIVE / RS AIM / RT FIRE / LT SPECIAL",
                ),
                36.0,
                size.y - 72.0,
                20.0,
                MUTED,
            );
            return None;
        }
        text("LOCAL LOBBY", 36.0, 64.0, 30.0, INK);
        text("You + one AI opponent", 36.0, 98.0, 20.0, MUTED);
        if button(
            Rect::new(size.x - 196.0, 36.0, 160.0, 48.0),
            prompt("BACK", "BACK / B"),
            mouse,
            false,
        ) || crate::controller_ui::back()
        {
            self.screen = Screen::Home;
            return None;
        }
        for (i, label) in self.map_choices().into_iter().enumerate() {
            let rect = Rect::new(36.0, 140.0 + i as f32 * 56.0, 328.0, 48.0);
            if button(rect, label, mouse, self.source == i) {
                self.select_map(i);
            }
        }
        if button(
            Rect::new(36.0, 340.0, 328.0, 44.0),
            "IMPORT MAP",
            mouse,
            false,
        ) && let Err(e) = crate::level_storage::pick_import()
        {
            self.status = e;
        }
        self.preview(Rect::new(404.0, 140.0, size.x - 440.0, 242.0));
        text(&self.map.name, 404.0, 417.0, 22.0, INK);
        text(
            &format!(
                "{:.0} x {:.0} m / {} objects",
                self.map.size[0],
                self.map.size[1],
                self.map.items.len()
            ),
            404.0,
            451.0,
            19.0,
            MUTED,
        );
        let width = (size.x - 96.0) / 3.0;
        let y = size.y - 206.0;
        if button(
            Rect::new(36.0, y, width, 48.0),
            &format!("YOUR TEAM / {}", self.config.team.label()),
            mouse,
            false,
        ) {
            self.config.team = self.config.team.other();
        }
        if button(
            Rect::new(48.0 + width, y, width, 48.0),
            &format!(
                "TIME / {}",
                match self.config.time_limit_seconds {
                    Some(300) => "5 MIN",
                    Some(_) => "10 MIN",
                    None => "UNLIMITED",
                }
            ),
            mouse,
            false,
        ) {
            self.config.time_limit_seconds = match self.config.time_limit_seconds {
                Some(600) => None,
                None => Some(300),
                _ => Some(600),
            };
        }
        if button(
            Rect::new(60.0 + width * 2.0, y, width, 48.0),
            &format!(
                "AI / {}",
                if self.config.opponent == EnemyActivity::Conquer {
                    "CAPTURE"
                } else {
                    "HUNT"
                }
            ),
            mouse,
            false,
        ) {
            self.config.opponent = if self.config.opponent == EnemyActivity::Conquer {
                EnemyActivity::Attack
            } else {
                EnemyActivity::Conquer
            };
        }
        wrapped(
            if self.status.is_empty() {
                "Capture the enemy flag. Tanks respawn after 3 seconds."
            } else {
                &self.status
            },
            36.0,
            size.y - 118.0,
            size.x - 380.0,
            19.0,
            MUTED,
        );
        if button(
            Rect::new(size.x - 316.0, size.y - 120.0, 280.0, 54.0),
            "START MATCH",
            mouse,
            true,
        ) {
            return Some(Action::Start);
        }
        None
    }
}
