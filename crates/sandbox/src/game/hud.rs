use super::Action;
use super::*;
use crate::ui::*;
use crate::ui::{PINK, VIOLET};
use sim::{Ammunition, level::Team, matchplay::Outcome};

fn clock(seconds: f32) -> String {
    let seconds = seconds.ceil() as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
fn center_text(label: &str, x: f32, y: f32, size: f32, color: Color) {
    let width = measure_text(label, None, size as u16, 1.0).width;
    text(label, x - width * 0.5, y, size, color);
}
impl Game {
    pub fn captures(&self, mouse: Vec2) -> bool {
        self.help && Rect::new(24.0, 104.0, 500.0, 204.0).contains(mouse)
    }
    pub fn draw_hud(&mut self, size: Vec2, mouse: Vec2, lab: &mut Lab) -> Option<Action> {
        let progress = lab.match_progress.as_ref().expect("match session");
        let network = lab.world.is_replica();
        let network_host = self.network_host();
        let outcome = progress.outcome;
        let team = progress.config.team;
        let losses = progress.tanks_lost;
        let elapsed = progress.elapsed_seconds();
        let time = progress.remaining_seconds().unwrap_or(elapsed);
        let team_color = if team == Team::Blue { CYAN } else { PINK };
        let modal = self.menu || outcome.is_some() || self.countdown > 0.0;
        draw_rectangle(0.0, 0.0, size.x, 80.0, PAPER);
        draw_line(24.0, 79.0, size.x - 24.0, 79.0, 1.0, RULE);
        text("F R A C T U R E", 32.0, 36.0, 25.0, INK);
        text(
            &format!("{} TEAM / CAPTURE THE FLAG", team.label()),
            32.0,
            62.0,
            15.0,
            team_color,
        );
        center_text(&clock(time), size.x * 0.5, 43.0, 32.0, INK);
        center_text("FIRST CAPTURE WINS", size.x * 0.5, 66.0, 13.0, MUTED);
        if !modal
            && button(
                Rect::new(size.x - 190.0, 23.0, 166.0, 38.0),
                "ESC  MENU",
                mouse,
                false,
            )
        {
            self.menu = true;
        }
        let y = size.y - 97.0;
        draw_rectangle(0.0, y, size.x, 97.0, PAPER);
        draw_line(24.0, y, size.x - 24.0, y, 1.0, RULE);
        let health = lab
            .world
            .tanks()
            .find(|t| t.id == lab.world.player_tank_id())
            .map_or(0.0, |t| t.health);
        text(
            &format!("HULL {:03.0}", health),
            32.0,
            y + 32.0,
            21.0,
            team_color,
        );
        draw_rectangle(32.0, y + 45.0, 180.0, 7.0, RULE);
        draw_rectangle(32.0, y + 45.0, 180.0 * health / 100.0, 7.0, team_color);
        text("WASD DRIVE / H HELP", 32.0, y + 79.0, 15.0, MUTED);
        text("SPACE FIRE / G SPECIAL", 265.0, y + 32.0, 17.0, INK);
        text("5 / 6 / B CHANGE ROUND", 265.0, y + 65.0, 16.0, MUTED);
        text(
            &format!("TANKS LOST  BLUE {} / RED {}", losses[0], losses[1]),
            size.x * 0.44,
            y + 32.0,
            16.0,
            MUTED,
        );
        let special = if lab.world.plasma_remaining() > 0.0 {
            format!("PLASMA / CHARGING {:.1}s", lab.world.plasma_remaining())
        } else if lab.world.laser_remaining() > 0.0 {
            format!("LASER / {:.1}s", lab.world.laser_remaining())
        } else if let Some(weapon) = lab.world.carried_special() {
            if lab.world.special_cooldown() > 0.0 {
                format!(
                    "{} x{} / {:.1}s",
                    weapon.label(),
                    lab.world.special_charges(),
                    lab.world.special_cooldown()
                )
            } else {
                format!(
                    "{} x{} / READY",
                    weapon.label(),
                    lab.world.special_charges()
                )
            }
        } else {
            "SPECIAL / FIND A PICKUP".into()
        };
        text(&special, size.x * 0.44, y + 65.0, 16.0, VIOLET);
        for (i, ammo) in Ammunition::MAIN.into_iter().enumerate() {
            let rect = Rect::new(size.x - 275.0 + i as f32 * 120.0, y + 10.0, 110.0, 36.0);
            if !modal {
                if button(rect, ammo.label(), mouse, lab.inspector.ammunition == ammo) {
                    lab.select_ammunition(ammo);
                }
            } else {
                text(
                    ammo.label(),
                    rect.x + 12.0,
                    rect.y + 26.0,
                    18.0,
                    if lab.inspector.ammunition == ammo {
                        CYAN
                    } else {
                        MUTED
                    },
                );
            }
        }
        right_text(
            if !lab.world.tank_alive() {
                "TANK DESTROYED"
            } else if lab.world.special_active() {
                "SPECIAL ACTIVE"
            } else if lab.world.reload_remaining() > 0.0 {
                "RELOADING"
            } else {
                "MAIN GUN READY"
            },
            size.x - 40.0,
            y + 76.0,
            17.0,
            CYAN,
        );
        if self.help && !modal {
            draw_rectangle(24.0, 104.0, 500.0, 204.0, PAPER);
            draw_rectangle_lines(24.0, 104.0, 500.0, 204.0, 1.0, RULE);
            for (i, line) in [
                "WASD / MOVE & STEER",
                "MOUSE / Q E / J K / AIM TURRET",
                "SPACE/LMB/F FIRE / G/RMB SPECIAL",
                "5 / 6 / B / CHANGE MAIN-GUN ROUND",
                "WHEEL / ZOOM     ESC / MATCH MENU",
                "TOUCH THE ENEMY FLAG TO WIN",
            ]
            .into_iter()
            .enumerate()
            {
                text(line, 44.0, 136.0 + i as f32 * 29.0, 17.0, INK);
            }
        }
        if let Some(outcome) = outcome {
            let title = match outcome {
                Outcome::Captured(winner) if winner == team => "VICTORY",
                Outcome::Captured(_) => "DEFEAT",
                Outcome::TimeLimit => "DRAW",
            };
            self.panel(size, title);
            center_text(
                match outcome {
                    Outcome::Captured(winner) if winner == team => "YOU CAPTURED THE ENEMY FLAG",
                    Outcome::Captured(_) => "THE ENEMY TEAM CAPTURED YOUR FLAG",
                    Outcome::TimeLimit => "TIME LIMIT REACHED / NO FLAG CAPTURED",
                },
                size.x * 0.5,
                size.y * 0.5 - 40.0,
                18.0,
                MUTED,
            );
            center_text(
                &format!(
                    "{} PLAYED / {} SHOTS",
                    clock(elapsed),
                    lab.world.stats().shots
                ),
                size.x * 0.5,
                size.y * 0.5 + 7.0,
                20.0,
                INK,
            );
            center_text(
                &format!("TANKS LOST / BLUE {} / RED {}", losses[0], losses[1]),
                size.x * 0.5,
                size.y * 0.5 + 43.0,
                18.0,
                MUTED,
            );
            if (!network || network_host)
                && button(
                    Rect::new(size.x * 0.5 - 248.0, size.y * 0.5 + 92.0, 230.0, 44.0),
                    if network { "MATCH LOBBY" } else { "REMATCH" },
                    mouse,
                    true,
                )
            {
                return Some(Action::Start);
            }
            if button(
                Rect::new(size.x * 0.5 + 18.0, size.y * 0.5 + 92.0, 230.0, 44.0),
                if network {
                    "LEAVE SESSION"
                } else {
                    "RETURN TO LOBBY"
                },
                mouse,
                false,
            ) {
                return Some(Action::Leave);
            }
        } else if self.menu {
            self.panel(
                size,
                if network {
                    "MATCH MENU"
                } else {
                    "MATCH PAUSED"
                },
            );
            center_text(
                if network {
                    "LAN MATCH / PLAY CONTINUES"
                } else {
                    "LOCAL MATCH / THE CLOCK IS PAUSED"
                },
                size.x * 0.5,
                size.y * 0.5 - 40.0,
                17.0,
                MUTED,
            );
            for (i, (label, action)) in [
                ("RESUME", Action::Resume),
                (
                    if network {
                        "MATCH LOBBY (HOST)"
                    } else {
                        "RESTART MATCH"
                    },
                    Action::Start,
                ),
                (
                    if network {
                        "LEAVE SESSION"
                    } else {
                        "RETURN TO LOBBY"
                    },
                    Action::Leave,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                if network && !network_host && matches!(action, Action::Start) {
                    continue;
                }
                if button(
                    Rect::new(
                        size.x * 0.5 - 220.0,
                        size.y * 0.5 - 10.0 + i as f32 * 52.0,
                        440.0,
                        40.0,
                    ),
                    label,
                    mouse,
                    i == 0,
                ) {
                    return Some(action);
                }
            }
        } else if self.countdown > 0.0 {
            center_text(
                &format!("{:.0}", self.countdown.ceil()),
                size.x * 0.5,
                size.y * 0.5,
                88.0,
                INK,
            );
            center_text(
                "CAPTURE THE ENEMY FLAG",
                size.x * 0.5,
                size.y * 0.5 + 48.0,
                22.0,
                CYAN,
            );
        }
        None
    }
    fn panel(&self, size: Vec2, title: &str) {
        draw_rectangle(
            0.0,
            80.0,
            size.x,
            size.y - 177.0,
            Color::new(0.01, 0.02, 0.04, 0.8),
        );
        let rect = Rect::new(size.x * 0.5 - 320.0, size.y * 0.5 - 175.0, 640.0, 340.0);
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, CYAN);
        center_text(title, size.x * 0.5, rect.y + 69.0, 38.0, INK);
    }
}
