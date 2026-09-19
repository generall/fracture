//! A focused picker, kept outside the mesh/property sidebar.
use crate::{editor_storage as storage, ui::*};
use macroquad::prelude::*;
use sim::{LibraryPreset, ObjectAsset, ObjectLibrary};

pub enum LibraryAction {
    Load(Box<ObjectAsset>),
    Yard,
    BarrelShot,
    Crush,
}
pub struct Library {
    pub open: bool,
    objects: ObjectLibrary,
    saved_tab: bool,
    page: usize,
    starter_page: usize,
    status: String,
    restore_error: Option<String>,
}
impl Library {
    pub fn new() -> Self {
        let restored = storage::restore_library()
            .and_then(|json| json.map(|j| ObjectLibrary::from_json(&j)).transpose());
        let (objects, restore_error) = match restored {
            Ok(objects) => (objects.unwrap_or_default(), None),
            Err(e) => (ObjectLibrary::default(), Some(e)),
        };
        Self {
            open: false,
            objects,
            saved_tab: false,
            page: 0,
            starter_page: 0,
            status: "Choose a starter, edit it, then save a named copy.".into(),
            restore_error,
        }
    }
    pub fn len(&self) -> usize {
        self.objects.objects.len()
    }
    fn persist(&mut self, next: ObjectLibrary) {
        if let Some(e) = &self.restore_error {
            self.status =
                format!("Saved library could not be read: {e} Export your draft from File.");
            return;
        }
        match next.to_json().and_then(|j| storage::persist_library(&j)) {
            Ok(()) => {
                self.objects = next;
                self.status = "Library saved on this device.".into();
            }
            Err(e) => self.status = e,
        }
    }
    pub fn draw(
        &mut self,
        size: Vec2,
        mouse: Vec2,
        current: &ObjectAsset,
    ) -> Option<LibraryAction> {
        let r = Rect::new(
            (size.x - 1080.0) * 0.5,
            (size.y - 680.0) * 0.5,
            1080.0,
            680.0,
        );
        draw_rectangle(r.x, r.y, r.w, r.h, PAPER);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, RULE);
        text("OBJECT LIBRARY", r.x + 24.0, r.y + 43.0, 28.0, INK);
        if button(
            Rect::new(r.right() - 134.0, r.y + 18.0, 110.0, 38.0),
            "CLOSE",
            mouse,
            false,
        ) || is_key_pressed(KeyCode::Escape)
            || is_key_pressed(KeyCode::L)
        {
            self.open = false;
            return None;
        }
        for (i, label) in ["STARTERS", "SAVED COPIES"].into_iter().enumerate() {
            if button(
                Rect::new(r.x + 24.0 + i as f32 * 200.0, r.y + 70.0, 190.0, 38.0),
                label,
                mouse,
                self.saved_tab == (i == 1),
            ) {
                self.saved_tab = i == 1;
                self.page = 0;
                self.starter_page = 0;
            }
        }
        if self.saved_tab {
            let pages = self.len().div_ceil(6).max(1);
            self.page = self.page.min(pages - 1);
            if self.len() == 0 {
                wrapped(
                    "Your saved objects will appear here. Rename the current draft in Shape, then use Save current. Matching names update the existing copy.",
                    r.x + 32.0,
                    r.y + 174.0,
                    760.0,
                    20.0,
                    MUTED,
                );
            }
            let mut remove = None;
            for (i, a) in self
                .objects
                .objects
                .iter()
                .enumerate()
                .skip(self.page * 6)
                .take(6)
            {
                let row = Rect::new(r.x + 24.0, r.y + 132.0 + (i % 6) as f32 * 60.0, 886.0, 50.0);
                if button(row, &a.name, mouse, false) {
                    self.open = false;
                    return Some(LibraryAction::Load(Box::new(a.clone())));
                }
                let color = asset_color(a);
                draw_rectangle(row.x + 9.0, row.y + 12.0, 4.0, 26.0, color);
                if button(
                    Rect::new(row.right() + 10.0, row.y, 136.0, 50.0),
                    "REMOVE",
                    mouse,
                    false,
                ) {
                    remove = Some(i);
                }
            }
            if let Some(i) = remove {
                let mut next = self.objects.clone();
                next.objects.remove(i);
                self.persist(next);
            }
            if button(
                Rect::new(r.x + 24.0, r.y + 510.0, 100.0, 36.0),
                "PREV",
                mouse,
                false,
            ) {
                self.page = self.page.saturating_sub(1);
            }
            text(
                &format!("{} / {}   -   {} saved", self.page + 1, pages, self.len()),
                r.x + 144.0,
                r.y + 535.0,
                18.0,
                MUTED,
            );
            if button(
                Rect::new(r.right() - 124.0, r.y + 510.0, 100.0, 36.0),
                "NEXT",
                mouse,
                false,
            ) {
                self.page = (self.page + 1).min(pages - 1);
            }
        } else {
            if button(
                Rect::new(r.right() - 274.0, r.y + 70.0, 250.0, 38.0),
                if self.starter_page == 0 {
                    "MORE STARTERS  >"
                } else {
                    "<  FIRST STARTERS"
                },
                mouse,
                false,
            ) {
                self.starter_page = 1 - self.starter_page;
            }
            for (i, preset) in LibraryPreset::ALL
                .into_iter()
                .skip(self.starter_page * 4)
                .take(4)
                .enumerate()
            {
                let card = Rect::new(
                    r.x + 24.0 + (i % 2) as f32 * 524.0,
                    r.y + 132.0 + (i / 2) as f32 * 200.0,
                    508.0,
                    184.0,
                );
                let a = preset.asset();
                let color = asset_color(&a);
                let hover = card.contains(mouse);
                draw_rectangle(
                    card.x,
                    card.y,
                    card.w,
                    card.h,
                    Color::new(0.035, 0.055, 0.09, 1.0),
                );
                draw_rectangle_lines(
                    card.x,
                    card.y,
                    card.w,
                    card.h,
                    if hover { 2.0 } else { 1.0 },
                    if hover { color } else { RULE },
                );
                preview(&a, vec2(card.x + 60.0, card.y + 86.0), color);
                text(preset.label(), card.x + 118.0, card.y + 38.0, 21.0, color);
                wrapped(
                    preset.description(),
                    card.x + 118.0,
                    card.y + 74.0,
                    362.0,
                    18.0,
                    INK,
                );
                text(
                    "OPEN IN EDITOR  >",
                    card.x + 118.0,
                    card.bottom() - 22.0,
                    15.0,
                    MUTED,
                );
                if hover && is_mouse_button_pressed(MouseButton::Left) {
                    self.open = false;
                    return Some(LibraryAction::Load(Box::new(a)));
                }
            }
        }
        draw_line(
            r.x + 24.0,
            r.y + 560.0,
            r.right() - 24.0,
            r.y + 560.0,
            1.0,
            RULE,
        );
        if button(
            Rect::new(r.x + 24.0, r.y + 580.0, 224.0, 40.0),
            "SAVE CURRENT",
            mouse,
            false,
        ) {
            let mut next = self.objects.clone();
            next.save(current.clone());
            self.persist(next);
        }
        if self.saved_tab {
            text(
                &format!("Draft: {}", current.name),
                r.x + 272.0,
                r.y + 606.0,
                18.0,
                INK,
            );
        } else if button(
            Rect::new(r.x + 272.0, r.y + 580.0, 310.0, 40.0),
            "SHOT RESPONSE / 6",
            mouse,
            false,
        ) {
            self.open = false;
            return Some(LibraryAction::BarrelShot);
        }
        if button(
            Rect::new(r.x + 596.0, r.y + 580.0, 216.0, 40.0),
            "DEFORMATION / 6",
            mouse,
            false,
        ) {
            self.open = false;
            return Some(LibraryAction::Crush);
        }
        if button(
            Rect::new(r.right() - 254.0, r.y + 580.0, 230.0, 40.0),
            "TEST OBJECT YARD",
            mouse,
            false,
        ) {
            self.open = false;
            return Some(LibraryAction::Yard);
        }
        wrapped(
            self.restore_error.as_deref().unwrap_or(&self.status),
            r.x + 24.0,
            r.y + 650.0,
            r.w - 48.0,
            16.0,
            MUTED,
        );
        None
    }
}
pub fn asset_color(a: &ObjectAsset) -> Color {
    a.color
        .map_or(CYAN, |[r, g, b]| Color::from_rgba(r, g, b, 255))
}
fn preview(a: &ObjectAsset, center: Vec2, color: Color) {
    let min = a
        .outline
        .iter()
        .fold(Vec2::splat(f32::INFINITY), |v, p| v.min(Vec2::from(*p)));
    let max = a
        .outline
        .iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), |v, p| v.max(Vec2::from(*p)));
    let scale = 84.0 / (max - min).max_element();
    for i in 0..a.outline.len() {
        let p = center + (Vec2::from(a.outline[i]) - (min + max) * 0.5) * scale;
        let q =
            center + (Vec2::from(a.outline[(i + 1) % a.outline.len()]) - (min + max) * 0.5) * scale;
        draw_line(p.x, p.y, q.x, q.y, 2.0, color);
    }
}
