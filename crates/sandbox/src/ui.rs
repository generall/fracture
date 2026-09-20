use macroquad::prelude::*;
use sim::{Ammunition, Scene, World};

pub const PAPER: Color = Color::new(0.025, 0.032, 0.075, 1.0);
pub const INK: Color = Color::new(0.91, 0.96, 1.0, 1.0);
pub const MUTED: Color = Color::new(0.58, 0.68, 0.82, 1.0);
pub const RULE: Color = Color::new(0.16, 0.23, 0.37, 1.0);
pub const CYAN: Color = Color::new(0.30, 0.92, 1.0, 1.0);
pub const PINK: Color = Color::new(1.0, 0.35, 0.67, 1.0);
pub const VIOLET: Color = Color::new(0.64, 0.53, 1.0, 1.0);

pub enum Action {
    Scene(Scene),
    Reset,
    Pause,
    Slow,
    Overlay,
    Fog,
    Step,
    Editor,
    LevelEditor,
    Ammunition(Ammunition),
}
pub struct Inspector {
    pub ammunition: Ammunition,
    pub mass: f32,
    pub speed: f32,
    pub help: bool,
}
impl Default for Inspector {
    fn default() -> Self {
        Self {
            ammunition: Ammunition::HighExplosive,
            mass: 6.0,
            speed: 220.0,
            help: false,
        }
    }
}
pub fn text(value: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        value,
        x,
        y,
        TextParams {
            font_size: size as u16,
            color,
            ..Default::default()
        },
    );
}
pub fn right_text(value: &str, right: f32, y: f32, size: f32, color: Color) {
    text(
        value,
        right - measure_text(value, None, size as u16, 1.0).width,
        y,
        size,
        color,
    );
}
pub fn button(rect: Rect, label: &str, mouse: Vec2, selected: bool) -> bool {
    let hover = rect.contains(mouse);
    if hover || selected {
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(0.08, 0.13, 0.22, 0.95),
        );
        draw_line(
            rect.x,
            rect.bottom(),
            rect.right(),
            rect.bottom(),
            2.0,
            CYAN,
        );
    }
    text(
        label,
        rect.x + 12.0,
        rect.y + 26.0,
        18.0,
        if selected || hover { CYAN } else { MUTED },
    );
    hover && is_mouse_button_pressed(MouseButton::Left)
}
impl Inspector {
    pub fn captures(&self, mouse: Vec2, size: Vec2) -> bool {
        mouse.y < 82.0
            || mouse.y > size.y - 100.0
            || (self.help && Rect::new(24.0, 100.0, 475.0, 490.0).contains(mouse))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        size: Vec2,
        mouse: Vec2,
        world: &World,
        paused: bool,
        slow: bool,
        overlay: bool,
        tick_ms: f64,
        fog: bool,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        draw_rectangle(0.0, 0.0, size.x, 80.0, PAPER);
        draw_line(24.0, 79.0, size.x - 24.0, 79.0, 1.0, RULE);
        draw_rectangle(24.0, 22.0, 4.0, 32.0, CYAN);
        text("F R A C T U R E", 42.0, 44.0, 28.0, INK);
        text("PROVING GROUND", 43.0, 66.0, 14.0, MUTED);
        let start = 330.0;
        let scene_width = (size.x - 700.0) / crate::controls::SCENES.len() as f32;
        for (i, binding) in crate::controls::SCENES.iter().enumerate() {
            let (scene, label) = (binding.scene, binding.label);
            if button(
                Rect::new(
                    start + i as f32 * scene_width,
                    23.0,
                    scene_width - 6.0,
                    38.0,
                ),
                label,
                mouse,
                world.scene() == scene,
            ) {
                actions.push(Action::Scene(scene));
            }
        }
        if button(
            Rect::new(size.x - 350.0, 23.0, 145.0, 38.0),
            "O  EDITOR",
            mouse,
            false,
        ) {
            actions.push(Action::Editor);
        }
        if button(
            Rect::new(size.x - 189.0, 23.0, 165.0, 38.0),
            if paused {
                "SPACE  RESUME"
            } else {
                "SPACE  PAUSE"
            },
            mouse,
            paused,
        ) {
            actions.push(Action::Pause);
        }
        text(world.scene().description(), 32.0, 113.0, 19.0, MUTED);
        right_text(
            if slow {
                "TIME  /  0.2x"
            } else {
                "TIME  /  1.0x"
            },
            size.x - 32.0,
            113.0,
            17.0,
            if slow { PINK } else { MUTED },
        );
        if overlay {
            text(
                "STRESS VIEW   CYAN > PINK > FAILURE",
                32.0,
                141.0,
                17.0,
                CYAN,
            );
        }
        let y = size.y - 97.0;
        draw_rectangle(0.0, y, size.x, 97.0, PAPER);
        draw_line(24.0, y, size.x - 24.0, y, 1.0, RULE);
        for (i, speed) in world.track_speeds().into_iter().enumerate() {
            let x = 32.0 + i as f32 * 135.0;
            text(
                if i == 0 { "L / Q Z" } else { "R / E C" },
                x,
                y + 26.0,
                17.0,
                CYAN,
            );
            for j in 0..10 {
                let active = j < (speed.abs() / 4.6 * 10.0).round() as usize;
                draw_rectangle(
                    x + j as f32 * 10.0,
                    y + 36.0,
                    6.0,
                    15.0,
                    if active { CYAN } else { RULE },
                );
            }
        }
        text(
            &format!(
                "WASD / {}",
                world.terrain_at(world.launcher_position()).label()
            ),
            32.0,
            size.y - 19.0,
            19.0,
            INK,
        );
        text("LMB / MAIN GUN   RMB / SPECIAL", 328.0, y + 32.0, 17.0, INK);
        if button(
            Rect::new(315.0, y + 48.0, 192.0, 36.0),
            "H  FIELD MANUAL",
            mouse,
            self.help,
        ) {
            self.help = !self.help;
        }
        if button(
            Rect::new(520.0, y + 48.0, 180.0, 36.0),
            "M LEVEL EDITOR",
            mouse,
            false,
        ) {
            actions.push(Action::LevelEditor);
        }
        let stats = world.stats();
        if world.scene() == Scene::Level {
            text(
                if fog {
                    "UNSEEN OBJECTS / LAST SEEN"
                } else {
                    "FULL MAP / TEST VIEW"
                },
                size.x * 0.5 + 30.0,
                y + 32.0,
                16.0,
                MUTED,
            );
            if button(
                Rect::new(size.x * 0.5 + 30.0, y + 45.0, 230.0, 36.0),
                if fog {
                    "V FOG OF WAR / ON"
                } else {
                    "V FOG OF WAR / OFF"
                },
                mouse,
                fog,
            ) {
                actions.push(Action::Fog);
            }
        } else {
            text(
                &format!("{:03} FRACTURES", stats.broken),
                size.x * 0.5 + 30.0,
                y + 32.0,
                20.0,
                VIOLET,
            );
            text(
                &if stats.ablated > 0 {
                    format!("{:03} CLEARED / {:03} CUT", stats.cleared, stats.ablated)
                } else {
                    format!("{:03} FRAGMENTS CLEARED", stats.cleared)
                },
                size.x * 0.5 + 30.0,
                y + 65.0,
                16.0,
                MUTED,
            );
        }
        let ready =
            world.tank_alive() && world.reload_remaining() <= 0.0 && world.laser_remaining() <= 0.0;
        for (i, ammunition) in Ammunition::MAIN.into_iter().enumerate() {
            if button(
                Rect::new(size.x - 382.0 + i as f32 * 89.0, y + 5.0, 85.0, 36.0),
                &format!("{} {}", i + 5, ammunition.label()),
                mouse,
                self.ammunition == ammunition,
            ) {
                actions.push(Action::Ammunition(ammunition));
            }
        }
        right_text(
            &if !world.tank_alive() {
                "TANK LOST / R TO REBUILD".into()
            } else {
                format!(
                    "{} / {} / {:.0} M/S",
                    if world.laser_remaining() > 0.0 {
                        "LASER ACTIVE"
                    } else if ready {
                        "READY"
                    } else {
                        "RELOAD"
                    },
                    if self.mass < 1.0 {
                        format!("{:.0} G", self.mass * 1000.0)
                    } else {
                        format!("{:.0} KG", self.mass)
                    },
                    self.speed
                )
            },
            size.x - 32.0,
            y + 64.0,
            17.0,
            if ready { CYAN } else { PINK },
        );
        let special_label = if world.laser_remaining() > 0.0 {
            format!("LASER / {:.1}s", world.laser_remaining())
        } else if let Some(weapon) = world.carried_special() {
            format!("RMB / {}", weapon.label())
        } else {
            "SPECIAL / EMPTY".into()
        };
        text(&special_label, size.x - 198.0, y + 29.0, 16.0, VIOLET);
        if paused {
            let label = "SIMULATION PAUSED  /  N TO STEP";
            let width = measure_text(label, None, 20, 1.0).width;
            draw_rectangle(
                (size.x - width) * 0.5 - 18.0,
                99.0,
                width + 36.0,
                38.0,
                PAPER,
            );
            text(label, (size.x - width) * 0.5, 125.0, 20.0, PINK);
        }
        if self.help {
            let rect = Rect::new(24.0, 100.0, 475.0, 490.0);
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, RULE);
            text("FIELD MANUAL", 44.0, 137.0, 26.0, CYAN);
            for (i, label) in [
                "W / S    Forward / reverse",
                "A / D    Differential steering",
                "Q / Z    Left track forward / reverse",
                "E / C    Right track forward / reverse",
                "RELEASE  Tracks brake automatically",
                "MOUSE    Aim turret independently",
                "CLICK/F  Main gun; RMB use special",
                if world.scene() == Scene::Level {
                    "WHEEL    Zoom / mouse look-ahead"
                } else {
                    "WHEEL    Zoom / middle-drag to pan"
                },
                "5–6 / B  Select ammo / cycle; HOME center",
            ]
            .iter()
            .enumerate()
            {
                text(label, 44.0, 173.0 + i as f32 * 28.0, 18.0, INK);
            }
            for (i, (label, action, selected)) in [
                ("R  REBUILD", Action::Reset, false),
                ("T  SLOW", Action::Slow, slow),
                ("TAB STRESS", Action::Overlay, overlay),
                ("N  STEP", Action::Step, false),
            ]
            .into_iter()
            .enumerate()
            {
                if button(
                    Rect::new(
                        40.0 + (i % 2) as f32 * 221.0,
                        427.0 + (i / 2) as f32 * 43.0,
                        212.0,
                        36.0,
                    ),
                    label,
                    mouse,
                    selected,
                ) {
                    actions.push(action);
                }
            }
            text(
                &format!("120 HZ / {tick_ms:.2} MS / {} BODIES", stats.bodies),
                44.0,
                550.0,
                16.0,
                MUTED,
            );
            text(
                &if stats.refinement_limited > 0 {
                    "DETAIL LIMIT REACHED / R TO REBUILD".into()
                } else if stats.unconverged > 0 {
                    "STRESS SOLVER LIMIT / TRY LESS DETAIL".into()
                } else {
                    format!(
                        "{} CHUNKS / {} CELLS / {} REFINED",
                        stats.chunks, stats.cells, stats.refinements
                    )
                },
                44.0,
                574.0,
                16.0,
                VIOLET,
            );
        }
        actions
    }
}
pub fn toast(rect: Rect, message: &str) {
    let width = measure_text(message, None, 18, 1.0).width + 36.0;
    let x = rect.center().x - width * 0.5;
    let y = rect.bottom() - 55.0;
    draw_rectangle(x, y, width, 36.0, PAPER);
    draw_rectangle_lines(x, y, width, 36.0, 1.0, PINK);
    text(message, x + 18.0, y + 24.0, 18.0, INK);
}

pub(crate) fn wrapped(value: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    let max = (width / (size * 0.61)) as usize;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    for (i, line) in lines.iter().take(2).enumerate() {
        text(line, x, y + i as f32 * 22.0, size, color);
    }
}
