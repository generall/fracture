//! Platform-neutral controller actions. SDL and browsers supply the same snapshot.
use macroquad::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(not(target_arch = "wasm32"))]
use native::Backend;
#[cfg(target_arch = "wasm32")]
use web::Backend;

pub const CONFIRM: u32 = 1 << 0;
pub const BACK: u32 = 1 << 1;
pub const HELP: u32 = 1 << 2;
pub const SETTINGS: u32 = 1 << 3;
pub const AMMO: u32 = 1 << 4;
pub const MENU: u32 = 1 << 5;
pub const FIRE: u32 = 1 << 6;
pub const SPECIAL: u32 = 1 << 7;
pub const UP: u32 = 1 << 8;
pub const DOWN: u32 = 1 << 9;
pub const LEFT: u32 = 1 << 10;
pub const RIGHT: u32 = 1 << 11;

#[derive(Clone, Copy, Default)]
pub struct Snapshot {
    pub connected: bool,
    pub left: Vec2,
    pub right: Vec2,
    pub buttons: u32,
}
#[derive(Clone, Copy, Default)]
pub struct Actions {
    pub connected: bool,
    pub disconnected: bool,
    pub drive: Vec2,
    pub aim: Vec2,
    pub held: u32,
    pub pressed: u32,
    pub navigation: Vec2,
}
impl Actions {
    pub fn pressed(self, action: u32) -> bool {
        self.pressed & action != 0
    }
    pub fn held(self, action: u32) -> bool {
        self.held & action != 0
    }
    pub fn used(self) -> bool {
        self.drive != Vec2::ZERO || self.aim != Vec2::ZERO || self.pressed != 0
    }
}
#[derive(Default)]
pub struct Tracker {
    previous: Snapshot,
    direction: Vec2,
    repeat: f32,
    armed: bool,
}
impl Tracker {
    pub fn update(&mut self, raw: Snapshot, dt: f32) -> Actions {
        let disconnected = self.previous.connected && !raw.connected;
        // A newly connected pad must first return to neutral. This prevents
        // held triggers/buttons from firing or accepting menus on reconnection.
        if !raw.connected || !self.previous.connected {
            self.armed = false;
        }
        let drive = dead_zone(raw.left);
        let aim = dead_zone(raw.right);
        if raw.connected && raw.buttons == 0 && drive == Vec2::ZERO && aim == Vec2::ZERO {
            self.armed = true;
        }
        let mut out = Actions {
            connected: raw.connected,
            disconnected,
            ..Actions::default()
        };
        if raw.connected && self.armed {
            out.drive = drive;
            out.aim = aim;
            out.held = raw.buttons;
            out.pressed = raw.buttons & !self.previous.buttons;
            let mut direction = vec2(
                ((raw.buttons & RIGHT != 0) as i32 - (raw.buttons & LEFT != 0) as i32) as f32,
                ((raw.buttons & DOWN != 0) as i32 - (raw.buttons & UP != 0) as i32) as f32,
            );
            if direction == Vec2::ZERO && drive.length() > 0.5 {
                direction = if drive.x.abs() > drive.y.abs() {
                    vec2(drive.x.signum(), 0.0)
                } else {
                    vec2(0.0, drive.y.signum())
                };
            }
            self.repeat -= dt;
            if direction != self.direction || (direction != Vec2::ZERO && self.repeat <= 0.0) {
                out.navigation = direction;
                self.repeat = if direction != self.direction {
                    0.35
                } else {
                    0.13
                };
            }
            self.direction = direction;
        } else {
            self.direction = Vec2::ZERO;
        }
        self.previous = raw;
        out
    }
}
pub fn dead_zone(value: Vec2) -> Vec2 {
    if !value.is_finite() {
        return Vec2::ZERO;
    }
    let length = value.length();
    if length <= 0.20 {
        Vec2::ZERO
    } else {
        value / length * ((length.min(1.0) - 0.20) / 0.80)
    }
}
struct Lifecycle {
    suspended: bool,
}
impl miniquad::EventHandler for Lifecycle {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn window_minimized_event(&mut self) {
        self.suspended = true;
    }
    fn window_restored_event(&mut self) {
        self.suspended = false;
    }
}
pub struct Controller {
    backend: Backend,
    tracker: Tracker,
    pub active: bool,
    last_mouse: Vec2,
    subscriber: usize,
    lifecycle: Lifecycle,
}
impl Controller {
    pub fn new() -> Self {
        Self {
            backend: Backend::new(),
            tracker: Tracker::default(),
            active: false,
            last_mouse: Vec2::ZERO,
            subscriber: macroquad::input::utils::register_input_subscriber(),
            lifecycle: Lifecycle { suspended: false },
        }
    }
    pub fn poll(&mut self, dt: f32, handheld: bool) -> Actions {
        macroquad::input::utils::repeat_all_miniquad_input(&mut self.lifecycle, self.subscriber);
        let raw = self.backend.poll();
        let raw = if self.lifecycle.suspended {
            Snapshot::default()
        } else {
            raw
        };
        let actions = self.tracker.update(raw, dt);
        let mouse = Vec2::from(mouse_position());
        // Deck desktop mappings can emit mouse motion alongside stick input,
        // including after release. Require a deliberate handoff to the mouse.
        if (!handheld && (mouse - self.last_mouse).length_squared() > 4.0)
            || !get_keys_pressed().is_empty()
            || is_mouse_button_pressed(MouseButton::Left)
            || (handheld
                && [MouseButton::Right, MouseButton::Middle]
                    .into_iter()
                    .any(is_mouse_button_pressed))
        {
            self.active = false;
        }
        if actions.used() {
            self.active = true;
        }
        if !actions.connected {
            self.active = false;
        }
        self.last_mouse = mouse;
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dead_zone_retains_analog_precision_and_rejects_invalid_values() {
        assert_eq!(dead_zone(vec2(0.1, -0.1)), Vec2::ZERO);
        assert!((dead_zone(vec2(0.6, 0.0)).x - 0.5).abs() < 0.001);
        assert!((dead_zone(Vec2::ONE).length() - 1.0).abs() < 0.001);
        assert_eq!(dead_zone(vec2(f32::NAN, 0.0)), Vec2::ZERO);
    }
    #[test]
    fn reconnect_requires_neutral_and_disconnect_releases_every_action() {
        let mut t = Tracker::default();
        let mut s = Snapshot {
            connected: true,
            buttons: FIRE,
            ..Snapshot::default()
        };
        assert!(!t.update(s, 0.016).held(FIRE));
        s.buttons = 0;
        t.update(s, 0.016);
        s.buttons = FIRE;
        assert!(t.update(s, 0.016).pressed(FIRE));
        assert!(!t.update(s, 0.016).pressed(FIRE));
        let release = t.update(Snapshot::default(), 0.016);
        assert!(release.disconnected);
        assert_eq!(release.held, 0);
        assert!(!t.update(s, 0.016).held(FIRE));
    }
    #[test]
    fn menu_direction_repeats_without_repeating_confirm() {
        let mut t = Tracker::default();
        let mut s = Snapshot {
            connected: true,
            ..Snapshot::default()
        };
        t.update(s, 0.016);
        s.buttons = DOWN | CONFIRM;
        assert_eq!(t.update(s, 0.016).navigation, Vec2::Y);
        let held = t.update(s, 0.1);
        assert_eq!(held.navigation, Vec2::ZERO);
        assert_eq!(held.pressed, 0);
        assert_eq!(t.update(s, 0.3).navigation, Vec2::Y);
    }
}
