use super::*;
impl Audio {
    /// A dedicated input frame prevents clicks/keys passing through into play.
    pub(crate) fn settings_frame(&mut self) -> bool {
        let frame = self.panel_open || is_key_pressed(KeyCode::F7);
        if is_key_pressed(KeyCode::F7) {
            self.panel_open = !self.panel_open;
        }
        if self.panel_open && is_key_pressed(KeyCode::Escape) {
            self.panel_open = false;
        }
        frame
    }
    pub(crate) fn draw_settings(&mut self, size: Vec2, mouse: Vec2) {
        use crate::ui::*;
        clear_background(PAPER);
        let x = size.x * 0.5 - 320.0;
        let y = size.y * 0.5 - 255.0;
        text("SOUND SETTINGS", x, y + 30.0, 30.0, INK);
        text(
            "Changes apply immediately and save on this device.",
            x,
            y + 62.0,
            16.0,
            MUTED,
        );
        let mut changed = false;
        if button(
            Rect::new(x, y + 85.0, 310.0, 38.0),
            if self.settings.muted {
                "SOUND / MUTED"
            } else {
                "SOUND / ON"
            },
            mouse,
            self.settings.muted,
        ) {
            self.settings.muted = !self.settings.muted;
            changed = true;
        }
        let rows = [
            ("MASTER VOLUME", &mut self.settings.master),
            ("MUSIC", &mut self.settings.music),
            ("BATTLE & VEHICLE EFFECTS", &mut self.settings.effects),
            ("PICKUP, RELOAD & UI CUES", &mut self.settings.feedback),
        ];
        for (i, (label, value)) in rows.into_iter().enumerate() {
            let ry = y + 158.0 + i as f32 * 65.0;
            text(label, x, ry, 16.0, INK);
            right_text(&format!("{}%", value), x + 640.0, ry, 16.0, CYAN);
            let track = Rect::new(x + 54.0, ry + 16.0, 532.0, 8.0);
            draw_rectangle(track.x, track.y, track.w, track.h, RULE);
            draw_rectangle(
                track.x,
                track.y,
                track.w * (*value as f32 / 100.0),
                track.h,
                CYAN,
            );
            draw_circle(
                track.x + track.w * (*value as f32 / 100.0),
                track.y + 4.0,
                6.0,
                CYAN,
            );
            if button(Rect::new(x, ry + 3.0, 38.0, 34.0), "-", mouse, false) {
                *value = value.saturating_sub(5);
                changed = true;
            }
            if button(
                Rect::new(x + 603.0, ry + 3.0, 38.0, 34.0),
                "+",
                mouse,
                false,
            ) {
                *value = (*value + 5).min(100);
                changed = true;
            }
            let hit = Rect::new(track.x, ry + 6.0, track.w, 28.0);
            if hit.contains(mouse) && is_mouse_button_down(MouseButton::Left) {
                let next = (((mouse.x - track.x) / track.w) * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u32;
                if next != *value {
                    *value = next;
                    changed = true;
                }
            }
        }
        if button(
            Rect::new(x, y + 435.0, 210.0, 40.0),
            "RESTORE DEFAULTS",
            mouse,
            false,
        ) {
            self.settings = settings::Settings::default();
            changed = true;
        }
        if button(
            Rect::new(x + 270.0, y + 435.0, 160.0, 40.0),
            "TEST EFFECT",
            mouse,
            false,
        ) {
            self.play(
                Clip::Collision,
                0.35 * self.settings.master_gain() * self.settings.effects_gain(),
            );
        }
        if button(
            Rect::new(x + 495.0, y + 435.0, 145.0, 40.0),
            "DONE / ESC",
            mouse,
            false,
        ) {
            self.panel_open = false;
        }
        text(
            "F7 SETTINGS   F8 MUTE   F9 MUSIC   F10 EFFECTS",
            x,
            y + 510.0,
            15.0,
            MUTED,
        );
        if changed {
            self.settings.save();
            self.stop_effects();
        }
    }
}
