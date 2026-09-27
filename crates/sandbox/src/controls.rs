//! One scene catalog for both the HUD and shortcuts; differential-track input.
use macroquad::prelude::*;
use sim::Scene;

pub struct SceneBinding {
    pub key: KeyCode,
    pub scene: Scene,
    pub label: &'static str,
}
pub const SCENES: &[SceneBinding] = &[
    SceneBinding {
        key: KeyCode::Key1,
        scene: Scene::Masonry,
        label: "1 BARRIER",
    },
    SceneBinding {
        key: KeyCode::Key2,
        scene: Scene::BarrelRubble,
        label: "2 RUBBLE",
    },
    SceneBinding {
        key: KeyCode::Key3,
        scene: Scene::LooseBlocks,
        label: "3 ROCKS",
    },
    SceneBinding {
        key: KeyCode::Key4,
        scene: Scene::ObjectYard,
        label: "4 OBJECTS",
    },
    SceneBinding {
        key: KeyCode::Key9,
        scene: Scene::Terrain,
        label: "9 TERRAIN",
    },
];

pub fn tracks() -> [f32; 2] {
    let throttle = (is_key_down(KeyCode::W) as i32 - is_key_down(KeyCode::S) as i32) as f32;
    let steer = (is_key_down(KeyCode::D) as i32 - is_key_down(KeyCode::A) as i32) as f32;
    mix_tracks(throttle, steer)
}

pub fn mix_tracks(throttle: f32, steer: f32) -> [f32; 2] {
    let mut tracks = [throttle + steer, throttle - steer];
    let maximum = tracks[0].abs().max(tracks[1].abs()).max(1.0);
    tracks = tracks.map(|t| t / maximum);
    tracks
}

/// Stick drive in world directions: turn the nearer hull end toward the stick,
/// pivoting in place when it points sideways and reversing when it points behind.
pub fn stick_tracks(stick: Vec2, heading: f32) -> [f32; 2] {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    let error = (stick.to_angle() - heading + PI).rem_euclid(TAU) - PI;
    let nearer_end = if error.abs() > FRAC_PI_2 {
        error - PI.copysign(error)
    } else {
        error
    };
    let steer = (nearer_end / 30.0_f32.to_radians()).clamp(-1.0, 1.0);
    mix_tracks(error.cos(), steer).map(|t| t * stick.length())
}

pub fn turret_turn() -> Option<f32> {
    let left = is_key_down(KeyCode::Q) || is_key_down(KeyCode::J);
    let right = is_key_down(KeyCode::E) || is_key_down(KeyCode::K);
    (left || right).then_some((right as i32 - left as i32) as f32)
}

/// Keep keyboard aim independent of an idle cursor and of tank translation.
#[derive(Default)]
pub struct KeyboardAim {
    angle: Option<f32>,
}

impl KeyboardAim {
    pub fn update(&mut self, turn: Option<f32>, mouse_moved: bool, barrel_angle: f32, dt: f32) {
        if let Some(turn) = turn {
            let angle = self.angle.get_or_insert(barrel_angle);
            *angle = (*angle + turn * 60.0_f32.to_radians() * dt).rem_euclid(std::f32::consts::TAU);
        } else if mouse_moved {
            self.angle = None;
        }
    }

    pub fn point_stick(&mut self, direction: Vec2) {
        if direction.length_squared() > 0.0 {
            self.angle = Some(direction.to_angle());
        }
    }

    pub fn active(&self) -> bool {
        self.angle.is_some()
    }

    pub fn target(&self, origin: Vec2) -> Option<Vec2> {
        self.angle
            .map(|angle| origin + Vec2::from_angle(angle) * 40.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_drives_toward_its_world_direction() {
        let [l, r] = stick_tracks(Vec2::X, 0.0);
        assert!(l > 0.99 && (l - r).abs() < 0.001, "aligned: straight ahead");
        let [l, r] = stick_tracks(-Vec2::X, 0.0);
        assert!(
            l < -0.99 && (l - r).abs() < 0.001,
            "behind: straight reverse"
        );
        // Positive steer (left track faster) increases the heading angle.
        let [l, r] = stick_tracks(Vec2::Y, 0.0);
        assert!(
            l > 0.0 && (l + r).abs() < 0.001,
            "sideways: pivot toward it"
        );
        let [l, r] = stick_tracks(Vec2::from_angle(-0.2), 0.0);
        assert!(
            l > 0.0 && r > 0.0 && r > l,
            "slightly left: forward and turn left"
        );
        let [l, r] = stick_tracks(Vec2::from_angle(0.2) * 0.5, 0.0);
        assert!(l <= 0.5 + 0.001 && r > 0.0, "half stick: half speed");
    }

    #[test]
    fn keyboard_aim_survives_release_and_tank_movement() {
        let mut aim = KeyboardAim::default();
        aim.update(Some(1.0), false, 0.0, 0.5);
        let direction = Vec2::from_angle(30.0_f32.to_radians()) * 40.0;
        assert!((aim.target(Vec2::ZERO).unwrap() - direction).length() < 0.0001);
        aim.update(None, false, 0.0, 1.0);
        let position = vec2(8.0, -3.0);
        assert!((aim.target(position).unwrap() - position - direction).length() < 0.0001);
    }

    #[test]
    fn keyboard_takes_priority_until_mouse_moves_without_rotation_keys() {
        let mut aim = KeyboardAim::default();
        aim.update(Some(-1.0), true, 0.0, 0.5);
        assert!(aim.target(Vec2::ZERO).unwrap().y < 0.0);
        aim.update(Some(0.0), true, 0.0, 0.5);
        assert!(aim.active(), "opposing keys still keep keyboard aiming");
        aim.update(None, true, 0.0, 0.5);
        assert!(!aim.active());
        aim.update(Some(1.0), false, std::f32::consts::PI, 0.0);
        assert!((aim.target(Vec2::ZERO).unwrap() - vec2(-40.0, 0.0)).length() < 0.0001);
    }

    #[test]
    fn rotation_is_frame_rate_independent_and_freezes_when_paused() {
        let mut fast = KeyboardAim::default();
        let mut slow = KeyboardAim::default();
        for _ in 0..120 {
            fast.update(Some(1.0), false, 0.0, 1.0 / 120.0);
        }
        for _ in 0..30 {
            slow.update(Some(1.0), false, 0.0, 1.0 / 30.0);
        }
        let target = fast.target(Vec2::ZERO).unwrap();
        assert!((target - slow.target(Vec2::ZERO).unwrap()).length() < 0.001);
        fast.update(Some(1.0), false, 0.0, 0.0);
        assert_eq!(fast.target(Vec2::ZERO), Some(target));
    }
}
