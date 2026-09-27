use macroquad::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Settings {
    pub muted: bool,
    pub master: u32,
    pub music: u32,
    pub effects: u32,
    pub feedback: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            muted: false,
            master: 100,
            music: 65,
            effects: 100,
            feedback: 60,
        }
    }
}
impl Settings {
    pub fn load() -> Self {
        Self::decode(load())
    }
    fn decode(bits: u32) -> Self {
        if bits & 0xe000_0000 == 0x4000_0000 {
            Self {
                muted: bits & 1 != 0,
                master: ((bits >> 1) & 127).min(100),
                music: ((bits >> 8) & 127).min(100),
                effects: ((bits >> 15) & 127).min(100),
                feedback: ((bits >> 22) & 127).min(100),
            }
        } else if bits & 0xffff_ff00 == 0x100 {
            Self {
                muted: bits & 1 != 0,
                music: ((bits >> 1) & 7).min(4) * 25,
                effects: ((bits >> 4) & 7).min(4) * 25,
                ..Self::default()
            }
        } else {
            Self::default()
        }
    }
    fn encode(self) -> u32 {
        0x4000_0000
            | self.muted as u32
            | (self.master << 1)
            | (self.music << 8)
            | (self.effects << 15)
            | (self.feedback << 22)
    }
    pub fn save(self) {
        store(self.encode());
    }
    pub fn master_gain(self) -> f32 {
        // Process-only silence for automated native tests; never persist it in
        // the player's mixer settings. Browser tests mute Chrome's output.
        #[cfg(not(target_arch = "wasm32"))]
        {
            static SILENT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *SILENT.get_or_init(|| std::env::var("FRACTURE_SILENT").as_deref() == Ok("1")) {
                return 0.0;
            }
        }
        if self.muted {
            0.0
        } else {
            self.master as f32 / 100.0
        }
    }
    pub fn music_gain(self) -> f32 {
        self.music as f32 / 100.0
    }
    pub fn effects_gain(self) -> f32 {
        self.effects as f32 / 100.0
    }
    pub fn feedback_gain(self) -> f32 {
        self.feedback as f32 / 100.0
    }
    pub fn change(&mut self, slot: usize) {
        let decrease = |v: u32| if v == 0 { 100 } else { v.saturating_sub(10) };
        match slot {
            0 => self.muted = !self.muted,
            1 => self.music = decrease(self.music),
            _ => self.effects = decrease(self.effects),
        }
    }
    pub fn keys(&mut self) -> bool {
        let mut changed = false;
        for (i, key) in [KeyCode::F8, KeyCode::F9, KeyCode::F10]
            .into_iter()
            .enumerate()
        {
            if is_key_pressed(key) {
                self.change(i);
                changed = true;
            }
        }
        changed
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn load() -> u32 {
    std::fs::read_to_string("audio-settings.txt")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
#[cfg(not(target_arch = "wasm32"))]
fn store(value: u32) {
    let _ = std::fs::write("audio-settings.txt", value.to_string());
}
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn audio_settings_load() -> u32;
    fn audio_settings_store(value: u32);
}
#[cfg(target_arch = "wasm32")]
fn load() -> u32 {
    unsafe { audio_settings_load() }
}
#[cfg(target_arch = "wasm32")]
fn store(value: u32) {
    unsafe { audio_settings_store(value) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_and_legacy_migration() {
        for muted in [false, true] {
            for value in [0, 1, 37, 65, 100] {
                let s = Settings {
                    muted,
                    master: value,
                    music: 100 - value,
                    effects: value,
                    feedback: value,
                };
                assert_eq!(Settings::decode(s.encode()), s);
            }
        }
        let legacy = Settings::decode(0x100 | 1 | (3 << 1) | (2 << 4));
        assert!(legacy.muted);
        assert_eq!(legacy.music, 75);
        assert_eq!(legacy.effects, 50);
        assert_eq!(legacy.master, 100);
        assert_eq!(Settings::decode(u32::MAX), Settings::default());
        assert_eq!(Settings::decode(0), Settings::default());
    }
}
