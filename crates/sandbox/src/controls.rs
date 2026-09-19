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
        scene: Scene::Cantilever,
        label: "2 WEAK LINK",
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
    let mut tracks = [throttle + steer, throttle - steer];
    let maximum = tracks[0].abs().max(tracks[1].abs()).max(1.0);
    tracks = tracks.map(|t| t / maximum);
    if is_key_down(KeyCode::Q) || is_key_down(KeyCode::Z) {
        tracks[0] = (is_key_down(KeyCode::Q) as i32 - is_key_down(KeyCode::Z) as i32) as f32;
    }
    if is_key_down(KeyCode::E) || is_key_down(KeyCode::C) {
        tracks[1] = (is_key_down(KeyCode::E) as i32 - is_key_down(KeyCode::C) as i32) as f32;
    }
    tracks
}
