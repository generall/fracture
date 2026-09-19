//! File/view menus and explicit export actions.
use super::*;

impl Editor {
    pub(super) fn export(&mut self) {
        if self.commit_input() {
            self.status = match self.asset.to_json().and_then(|s| storage::export(&s)) {
                Ok(s) => s.into(),
                Err(e) => format!("Export rejected: {e}"),
            };
        }
    }
    pub(super) fn dropdown(&mut self, mouse: Vec2) {
        let Some(menu) = self.menu else {
            return;
        };
        if is_key_pressed(KeyCode::Escape) {
            self.menu = None;
            return;
        }
        let rect = match menu {
            Menu::File => Rect::new(390.0, 70.0, 310.0, 374.0),
            Menu::View => Rect::new(500.0, 70.0, 310.0, 182.0),
        };
        draw_rectangle(
            rect.x + 5.0,
            rect.y + 5.0,
            rect.w,
            rect.h,
            Color::new(0.0, 0.0, 0.02, 0.75),
        );
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, RULE);
        match menu {
            Menu::File => {
                text("NEW FROM SHAPE", rect.x + 16.0, 102.0, 15.0, MUTED);
                if button(
                    Rect::new(rect.x + 10.0, 394.0, 290.0, 36.0),
                    "OBJECT LIBRARY    L",
                    mouse,
                    false,
                ) {
                    self.menu = None;
                    self.library.open = true;
                }
                for (i, preset) in ObjectPreset::ALL.into_iter().enumerate() {
                    if button(
                        Rect::new(rect.x + 10.0, 114.0 + i as f32 * 40.0, 290.0, 36.0),
                        preset.label(),
                        mouse,
                        false,
                    ) {
                        self.checkpoint();
                        self.asset = ObjectAsset::workshop_preset(preset);
                        self.selected = 0;
                        self.select_tab(Tab::Shape);
                        self.input = None;
                        self.menu = None;
                        self.fit();
                        self.status = "Preset loaded. Undo returns to your previous object.".into();
                    }
                }
                draw_line(rect.x + 16.0, 289.0, rect.right() - 16.0, 289.0, 1.0, RULE);
                if button(
                    Rect::new(rect.x + 10.0, 302.0, 290.0, 36.0),
                    "IMPORT JSON",
                    mouse,
                    false,
                ) {
                    self.menu = None;
                    if let Err(e) = storage::pick_import() {
                        self.status = e;
                    }
                }
                if button(
                    Rect::new(rect.x + 10.0, 346.0, 290.0, 36.0),
                    "EXPORT JSON   CTRL S",
                    mouse,
                    false,
                ) {
                    self.menu = None;
                    self.export();
                }
            }
            Menu::View => {
                if button(
                    Rect::new(rect.x + 10.0, 84.0, 290.0, 36.0),
                    "SNAP TO 0.1 M",
                    mouse,
                    self.snap,
                ) {
                    self.snap = !self.snap;
                    self.menu = None;
                }
                if button(
                    Rect::new(rect.x + 10.0, 126.0, 290.0, 36.0),
                    "SHOW SOLVER MESH",
                    mouse,
                    self.show_mesh,
                ) {
                    self.show_mesh = !self.show_mesh;
                    self.menu = None;
                }
                if button(
                    Rect::new(rect.x + 10.0, 168.0, 290.0, 36.0),
                    "FIT OBJECT    HOME",
                    mouse,
                    false,
                ) {
                    self.fit();
                    self.menu = None;
                }
                text(
                    "Wheel zoom / right-drag pan",
                    rect.x + 16.0,
                    232.0,
                    15.0,
                    MUTED,
                );
            }
        }
        if is_mouse_button_pressed(MouseButton::Left)
            && !rect.contains(mouse)
            && !Rect::new(390.0, 23.0, 206.0, 38.0).contains(mouse)
        {
            self.menu = None;
        }
    }
}
