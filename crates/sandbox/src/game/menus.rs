use super::Action;
use super::*;
use crate::ui::{PINK, VIOLET};
use crate::{ui::*, view::Viewport};
use sim::level::{EnemyActivity, LevelItem, Team};

impl Game {
    pub fn draw_menu(&mut self, size: Vec2, mouse: Vec2) -> Option<Action> {
        #[cfg(not(target_arch = "wasm32"))]
        if self.screen == Screen::Lan {
            let mut lan = self.lan.take().unwrap_or_default();
            let action = lan.draw(self, size, mouse);
            self.lan = Some(lan);
            return action;
        }
        if self.screen == Screen::Home {
            return self.home(size, mouse);
        }
        if let Some(import) = crate::level_storage::take_import() {
            match import.and_then(|json| Level::from_json(&json)) {
                Ok(map) => self.accept_map(map),
                Err(e) => self.status = e,
            }
        }
        text("FRACTURE / LOCAL LOBBY", 64.0, 68.0, 30.0, INK);
        text("You + one AI opponent", 64.0, 103.0, 18.0, MUTED);
        if button(
            Rect::new(size.x - 206.0, 40.0, 142.0, 38.0),
            "BACK",
            mouse,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            self.screen = Screen::Home;
            return None;
        }
        draw_line(64.0, 130.0, size.x - 64.0, 130.0, 1.0, RULE);
        text("BATTLEFIELD", 64.0, 177.0, 18.0, CYAN);
        for (i, (label, available)) in [
            ("QUARRY CROSSING", true),
            ("RIVER VALLEY", true),
            ("EDITOR DRAFT", self.draft.is_some()),
            ("YOUR MAP", self.custom.is_some()),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = Rect::new(64.0, 200.0 + i as f32 * 62.0, 338.0, 48.0);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, RULE);
            if available {
                if button(rect, label, mouse, self.source == i) {
                    self.select_map(i);
                }
            } else {
                text(
                    &format!("{label} / NONE"),
                    rect.x + 12.0,
                    rect.y + 27.0,
                    16.0,
                    MUTED,
                );
            }
        }
        if button(
            Rect::new(64.0, 470.0, 160.0, 38.0),
            "CREATE MAP",
            mouse,
            false,
        ) {
            return Some(Action::CreateMap);
        }
        if button(
            Rect::new(232.0, 470.0, 170.0, 38.0),
            "IMPORT JSON",
            mouse,
            false,
        ) && let Err(e) = crate::level_storage::pick_import()
        {
            self.status = e;
        }
        if button(
            Rect::new(64.0, 516.0, 338.0, 38.0),
            "EDIT SELECTED MAP",
            mouse,
            false,
        ) {
            return Some(Action::EditMap);
        }
        wrapped(
            "Create a battlefield or bring in a map from the editor. Your editor draft stays separate.",
            76.0,
            580.0,
            310.0,
            17.0,
            MUTED,
        );
        self.preview(Rect::new(460.0, 167.0, size.x - 524.0, 322.0));
        text(&self.map.name, 460.0, 523.0, 22.0, INK);
        text(
            &format!(
                "{:.0} x {:.0} m  /  {} objects",
                self.map.size[0],
                self.map.size[1],
                self.map.items.len()
            ),
            460.0,
            551.0,
            16.0,
            MUTED,
        );
        let half = (size.x - 548.0) * 0.5;
        for (i, team) in [self.config.team, self.config.team.other()]
            .into_iter()
            .enumerate()
        {
            let x = 460.0 + i as f32 * (half + 24.0);
            draw_rectangle_lines(x, 578.0, half, 88.0, 1.0, RULE);
            text(
                if i == 0 {
                    "YOU / READY"
                } else {
                    "AI OPPONENT / READY"
                },
                x + 18.0,
                608.0,
                18.0,
                INK,
            );
            text(
                &format!("{} TEAM", team.label()),
                x + 18.0,
                642.0,
                18.0,
                if team == Team::Blue { CYAN } else { PINK },
            );
        }
        if button(
            Rect::new(64.0, 695.0, 338.0, 38.0),
            &format!("YOUR TEAM / {}", self.config.team.label()),
            mouse,
            false,
        ) {
            self.config.team = self.config.team.other();
        }
        if button(
            Rect::new(460.0, 695.0, 285.0, 38.0),
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
            Rect::new(780.0, 695.0, size.x - 844.0, 38.0),
            &format!(
                "AI / {}",
                if self.config.opponent == EnemyActivity::Conquer {
                    "CAPTURE THE FLAG"
                } else {
                    "HUNT THE PLAYER"
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
        text(
            "Capture the enemy flag to win. Tanks respawn after 3 seconds.",
            64.0,
            size.y - 115.0,
            18.0,
            MUTED,
        );
        if !self.status.is_empty() {
            wrapped(
                &self.status,
                64.0,
                size.y - 84.0,
                size.x - 410.0,
                16.0,
                PINK,
            );
        }
        if button(
            Rect::new(size.x - 310.0, size.y - 86.0, 246.0, 48.0),
            "START MATCH  >",
            mouse,
            true,
        ) {
            return Some(Action::Start);
        }
        None
    }
    fn home(&mut self, size: Vec2, mouse: Vec2) -> Option<Action> {
        self.preview(Rect::new(size.x * 0.49, 180.0, size.x * 0.45, 475.0));
        text("F R A C T U R E", 80.0, 236.0, 58.0, INK);
        text("TANK COMBAT", 85.0, 278.0, 21.0, CYAN);
        wrapped(
            "Break through cover. Capture the flag.",
            85.0,
            336.0,
            475.0,
            25.0,
            MUTED,
        );
        if button(
            Rect::new(80.0, 486.0, 440.0, 52.0),
            "ENTER LOCAL LOBBY  >",
            mouse,
            true,
        ) || is_key_pressed(KeyCode::Enter)
        {
            return Some(Action::EnterLobby);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if button(
            Rect::new(80.0, 605.0, 440.0, 52.0),
            "PLAY ON LOCAL NETWORK  >",
            mouse,
            false,
        ) {
            return Some(Action::EnterLan);
        }
        text(
            "One player. One AI rival. Your battlefield.",
            92.0,
            581.0,
            17.0,
            MUTED,
        );
        draw_line(
            80.0,
            size.y - 112.0,
            size.x - 80.0,
            size.y - 112.0,
            1.0,
            RULE,
        );
        text(
            "WASD DRIVE / Q E AIM / SPACE FIRE / G SPECIAL",
            80.0,
            size.y - 62.0,
            17.0,
            MUTED,
        );
        None
    }
    fn preview(&self, rect: Rect) {
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(0.035, 0.05, 0.085, 1.0),
        );
        let base = Viewport::new(rect, Vec2::ZERO, 1.0).scale;
        let fit = ((rect.w - 30.0) / self.map.size[0]).min((rect.h - 30.0) / self.map.size[1]);
        let view = Viewport::new(rect, Vec2::ZERO, fit / base);
        crate::terrain_view::draw(&self.terrain, &view);
        for item in &self.map.items {
            let color = match item {
                LevelItem::Spawn { team, .. } | LevelItem::Flag { team, .. } => {
                    if *team == Team::Blue { CYAN } else { PINK }
                }
                LevelItem::Object { asset, .. } => asset
                    .color
                    .map_or(VIOLET, |c| Color::from_rgba(c[0], c[1], c[2], 255)),
                LevelItem::Pickup { .. } => CYAN,
                LevelItem::Terrain { .. } | LevelItem::Bridge { .. } => continue,
                _ => MUTED,
            };
            let points: Vec<_> = item
                .world_outline()
                .into_iter()
                .map(|p| view.screen(vec2(p.x, p.y)))
                .collect();
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                draw_line(a.x, a.y, b.x, b.y, 1.1, color);
            }
            if matches!(item, LevelItem::Flag { .. }) {
                let p = item.position();
                let p = view.screen(vec2(p.x, p.y));
                draw_circle(p.x, p.y, 4.0, color);
            }
        }
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, RULE);
    }
}
