use super::Action;
use super::*;
use crate::ui::VIOLET;
use crate::ui::*;
use sim::{SpecialWeapon, matchplay::Outcome};

fn clock(seconds: f32) -> String {
    let seconds = seconds.ceil() as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
fn center_text(label: &str, x: f32, y: f32, size: f32, color: Color) {
    let width = measure_text(label, None, size as u16, 1.0).width;
    text(label, x - width * 0.5, y, size, color);
}
// Keep the final charge visible for the lifetime of an active energy weapon.
fn special_status(world: &sim::World) -> Option<(SpecialWeapon, String)> {
    if world.plasma_remaining() > 0.0 {
        Some((
            SpecialWeapon::Plasma,
            format!("CHARGING {:.1}s", world.plasma_remaining()),
        ))
    } else if world.laser_remaining() > 0.0 {
        Some((
            SpecialWeapon::Laser,
            format!("ACTIVE {:.1}s", world.laser_remaining()),
        ))
    } else {
        world.carried_special().map(|weapon| {
            let status = if world.special_cooldown() > 0.0 {
                format!("RELOAD {:.1}s", world.special_cooldown())
            } else if weapon == SpecialWeapon::Mine {
                crate::controller_ui::prompt("G / RMB TO PLACE", "LT TO PLACE").into()
            } else {
                "READY".into()
            };
            (weapon, status)
        })
    }
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
        let modal = self.menu || outcome.is_some() || self.countdown > 0.0;
        // Small, non-interactive overlays leave the battlefield and aiming intact.
        if !self.menu && outcome.is_none() {
            let timer = clock(time);
            let width = measure_text(&timer, None, 26, 1.0).width + 28.0;
            let x = (size.x - width) * 0.5;
            draw_rectangle(x, 16.0, width, 42.0, Color { a: 0.72, ..PAPER });
            center_text(&timer, size.x * 0.5, 45.0, 26.0, INK);
            if let Some((weapon, status)) = special_status(&lab.world) {
                let charges = format!("{} x{}", weapon.label(), lab.world.special_charges());
                let width = measure_text(&charges, None, 21, 1.0)
                    .width
                    .max(measure_text(&status, None, 17, 1.0).width)
                    + 28.0;
                let x = size.x - width - 16.0;
                draw_rectangle(x, 16.0, width, 66.0, Color { a: 0.72, ..PAPER });
                text(&charges, x + 14.0, 43.0, 21.0, VIOLET);
                text(&status, x + 14.0, 67.0, 17.0, INK);
            }
        }
        if !modal && !lab.world.tank_alive() && lab.world.player_respawn_remaining().is_some() {
            self.draw_deployment(size, mouse, lab);
            return None;
        }
        if self.help && !modal {
            draw_rectangle(24.0, 104.0, 500.0, 204.0, PAPER);
            draw_rectangle_lines(24.0, 104.0, 500.0, 204.0, 1.0, RULE);
            for (i, line) in [
                crate::controller_ui::prompt("WASD / MOVE & STEER", "LEFT STICK / MOVE & STEER"),
                if lab.world.player_vehicle() == sim::VehicleKind::Mortar {
                    crate::controller_ui::prompt(
                        "MOUSE DISTANCE / Q E / J K / RANGE",
                        "RIGHT STICK UP/DOWN / SHELL RANGE",
                    )
                } else {
                    crate::controller_ui::prompt(
                        "MOUSE / Q E / J K / AIM TURRET",
                        "RIGHT STICK / AIM TURRET",
                    )
                },
                crate::controller_ui::prompt(
                    "SPACE/LMB/F FIRE / G/RMB SPECIAL",
                    "RT / FIRE     LT / SPECIAL",
                ),
                match lab.world.player_vehicle() {
                    sim::VehicleKind::Scout => "SMALL GUN / HOLD FIRE / NO SPECIALS",
                    sim::VehicleKind::Mortar => "TURN THE HULL TO AIM THE FIXED GUN",
                    sim::VehicleKind::Tank => crate::controller_ui::prompt(
                        "5 / 6 / B / CHANGE MAIN-GUN ROUND",
                        "RB / CHANGE MAIN-GUN ROUND",
                    ),
                },
                crate::controller_ui::prompt(
                    "WHEEL / ZOOM     ESC / MATCH MENU",
                    "D-PAD UP/DOWN ZOOM / MENU PAUSE",
                ),
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
                size.y * 0.5
                    - if cfg!(target_arch = "wasm32") {
                        40.0
                    } else {
                        64.0
                    },
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
                #[cfg(not(target_arch = "wasm32"))]
                ("EXIT GAME", Action::Exit),
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
                        size.y * 0.5
                            + if cfg!(target_arch = "wasm32") {
                                -10.0 + i as f32 * 52.0
                            } else {
                                -34.0 + i as f32 * 48.0
                            },
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
        draw_rectangle(0.0, 0.0, size.x, size.y, Color::new(0.01, 0.02, 0.04, 0.8));
        let rect = Rect::new(size.x * 0.5 - 320.0, size.y * 0.5 - 175.0, 640.0, 340.0);
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, CYAN);
        center_text(title, size.x * 0.5, rect.y + 69.0, 38.0, INK);
    }
}
