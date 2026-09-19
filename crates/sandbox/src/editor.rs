//! Draft state, undo history and editor orchestration.
//! Fields, panels, menus and canvas interaction live in their own modules.
use crate::{
    editor_storage as storage,
    ui::{CYAN, INK, MUTED, PAPER, PINK, RULE, VIOLET, button, right_text, text},
};
use macroquad::prelude::*;
use sim::{GrainMode, ObjectAsset, ObjectMesh, ObjectPreset, Surface};

mod canvas;
mod diagnostics;
mod fields;
mod menus;
mod panels;
use crate::ui::wrapped;
#[cfg(target_arch = "wasm32")]
use diagnostics::publish;
pub use diagnostics::publish_closed;

pub enum EditorAction {
    Close,
    Yard,
    BarrelShot,
    Crush,
    Test(ObjectAsset),
    Compare(ObjectAsset),
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    Color,
    ProjectileResistance,
    ContactResistance,
    CrushForce,
    CrushResistance,
    ExplosionEnergy,
    ExplosionRadius,
    YieldStress,
    PlasticFlow,
    Hardening,
    PlasticLimit,
    Density,
    Thickness,
    Friction,
    Restitution,
    SoftBounce,
    BounceSpeed,
    Tension,
    Compression,
    Shear,
    Cohesion,
    X,
    Y,
    GrainWidth,
    GrainHeight,
    Seed,
    Young,
    Toughness,
}
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Shape,
    Fracture,
    Material,
}
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    File,
    View,
}

struct Input {
    field: Field,
    text: String,
    all: bool,
    rect: Rect,
}

pub struct Editor {
    pub active: bool,
    asset: ObjectAsset,
    cached: ObjectAsset,
    mesh: Result<ObjectMesh, String>,
    undo: Vec<ObjectAsset>,
    redo: Vec<ObjectAsset>,
    selected: usize,
    grain_edit: bool,
    tab: Tab,
    menu: Option<Menu>,
    detail_open: bool,
    material_open: bool,
    bounce_open: bool,
    behavior_open: bool,
    plastic_open: bool,
    library: crate::object_library::Library,
    dragging: bool,
    input: Option<Input>,
    snap: bool,
    show_mesh: bool,
    center: Vec2,
    zoom: f32,
    target_zoom: f32,
    fit_requested: bool,
    zoom_anchor: Option<(Vec2, Vec2)>,
    last_mouse: Vec2,
    status: String,
    saved: Option<ObjectAsset>,
    save_attempted: Option<ObjectAsset>,
}

impl Editor {
    pub fn new() -> Self {
        let mut asset = ObjectAsset::workshop_preset(ObjectPreset::Rock);
        let mut status = "Drag a vertex to reshape. Shift-click an edge to add one.".to_owned();
        let mut preserve_unreadable_draft = false;
        match storage::restore() {
            Ok(Some(json)) => match ObjectAsset::from_json(&json) {
                Ok(restored) => {
                    asset = restored;
                    status = "Restored your saved draft.".into();
                }
                Err(e) => {
                    status = format!("Could not restore draft: {e}");
                    preserve_unreadable_draft = true;
                }
            },
            Err(e) => {
                status = e;
                preserve_unreadable_draft = true;
            }
            Ok(None) => {}
        }
        let save_attempted = preserve_unreadable_draft.then(|| asset.clone());
        Self {
            active: false,
            mesh: asset.mesh(),
            cached: asset.clone(),
            asset,
            undo: vec![],
            redo: vec![],
            selected: 0,
            grain_edit: false,
            tab: Tab::Shape,
            menu: None,
            detail_open: false,
            material_open: false,
            bounce_open: false,
            behavior_open: false,
            plastic_open: false,
            library: crate::object_library::Library::new(),
            dragging: false,
            input: None,
            snap: true,
            show_mesh: false,
            center: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            fit_requested: true,
            zoom_anchor: None,
            last_mouse: Vec2::ZERO,
            status,
            saved: None,
            save_attempted,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.menu = None;
        self.library.open = false;
        self.input = None;
        self.dragging = false;
        while get_char_pressed().is_some() {}
    }

    pub fn use_variant(&mut self, asset: ObjectAsset) {
        self.checkpoint();
        self.asset = asset;
        self.selected = 0;
        self.select_tab(Tab::Shape);
        self.status = "Chosen variant loaded. Undo restores the previous draft.".into();
        self.open();
        self.fit();
        self.refresh();
    }

    fn selected_point(&self) -> [f32; 2] {
        if self.grain_edit {
            self.asset
                .layout
                .as_ref()
                .and_then(|l| l.sites.get(self.selected))
                .copied()
                .unwrap_or([0.0; 2])
        } else {
            self.asset.outline[self.selected.min(self.asset.outline.len() - 1)]
        }
    }

    fn set_point(&mut self, p: [f32; 2]) {
        if self.grain_edit {
            if let Some(point) = self
                .asset
                .layout
                .as_mut()
                .and_then(|l| l.sites.get_mut(self.selected))
            {
                *point = p;
            }
        } else {
            if self.asset.is_circular() {
                self.asset
                    .set_radius(sim::Vec2::from(p).length().clamp(0.15, 4.5));
            } else {
                self.asset.outline[self.selected] = p;
            }
        }
    }

    fn regenerate(&mut self, mode: GrainMode, size: [f32; 2], seed: u32) {
        self.asset.regenerate_grains(mode, size, seed);
        self.selected = 0;
        if mode != GrainMode::Stones {
            self.grain_edit = false;
        }
        self.status = "Grain layout regenerated. Undo restores your previous layout.".into();
    }

    fn add_grain(&mut self, p: Vec2) {
        if self
            .asset
            .layout
            .as_ref()
            .is_none_or(|l| l.mode != GrainMode::Stones || l.sites.len() >= 128)
        {
            return;
        }
        self.checkpoint();
        let sites = &mut self.asset.layout.as_mut().unwrap().sites;
        sites.push(p.to_array());
        self.selected = sites.len() - 1;
    }

    fn checkpoint(&mut self) {
        if self.undo.last() != Some(&self.asset) {
            self.undo.push(self.asset.clone());
        }
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self, redo: bool) {
        self.input = None;
        self.dragging = false;
        let (from, to) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(asset) = from.pop() {
            to.push(std::mem::replace(&mut self.asset, asset));
            if self.grain_edit
                && self
                    .asset
                    .layout
                    .as_ref()
                    .is_none_or(|l| l.mode != GrainMode::Stones)
            {
                self.grain_edit = false;
            }
            let count = if self.grain_edit {
                self.asset.layout.as_ref().unwrap().sites.len()
            } else {
                self.asset.outline.len()
            };
            self.selected = self.selected.min(count.saturating_sub(1));
            if self.tab == Tab::Fracture {
                self.select_tab(Tab::Fracture);
            }
            self.status = if redo { "Redid edit." } else { "Undid edit." }.into();
        }
    }

    fn refresh(&mut self) {
        if self.asset != self.cached {
            self.mesh = self.asset.mesh();
            self.cached = self.asset.clone();
        }
        if !self.dragging && self.mesh.is_ok() && self.save_attempted.as_ref() != Some(&self.asset)
        {
            self.save_attempted = Some(self.asset.clone());
            match self
                .asset
                .to_json()
                .and_then(|json| storage::persist(&json))
            {
                Ok(()) => {
                    self.saved = Some(self.asset.clone());
                }
                Err(e) => self.status = e,
            }
        }
    }

    pub fn draw(
        &mut self,
        size: Vec2,
        mouse: Vec2,
        dt: f32,
        pixel_scale: f32,
    ) -> Option<EditorAction> {
        if self.library.open {
            let action = self.library.draw(size, mouse, &self.asset);
            let result = match action {
                Some(crate::object_library::LibraryAction::Load(asset)) => {
                    self.use_variant(*asset);
                    self.behavior_open = false;
                    self.bounce_open = false;
                    self.plastic_open = false;
                    self.status="Library object loaded. Edit and save a named copy; Undo restores your draft.".into();
                    None
                }
                Some(crate::object_library::LibraryAction::Yard) => Some(EditorAction::Yard),
                Some(crate::object_library::LibraryAction::BarrelShot) => {
                    Some(EditorAction::BarrelShot)
                }
                Some(crate::object_library::LibraryAction::Crush) => Some(EditorAction::Crush),
                None => None,
            };
            #[cfg(target_arch = "wasm32")]
            publish(self);
            return result;
        }
        if let Some(import) = storage::take_import() {
            match import.and_then(|json| ObjectAsset::from_json(&json)) {
                Ok(asset) => {
                    self.checkpoint();
                    self.asset = asset;
                    self.select_tab(Tab::Shape);
                    self.selected = 0;
                    self.input = None;
                    self.menu = None;
                    self.status = "Imported object. Undo restores the previous draft.".into();
                    self.fit();
                }
                Err(e) => self.status = format!("Import rejected: {e}"),
            }
        }
        let editing = self.input.is_some();
        let menu_was_open = self.menu.is_some();
        let input_blocked = is_mouse_button_pressed(MouseButton::Left)
            && self.input.as_ref().is_some_and(|i| !i.rect.contains(mouse))
            && !self.commit_input();
        let ctrl = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || is_key_down(KeyCode::LeftSuper)
            || is_key_down(KeyCode::RightSuper);
        if !editing && !menu_was_open {
            if is_key_pressed(KeyCode::L) && !ctrl {
                self.library.open = true;
            }
            if ctrl && is_key_pressed(KeyCode::Z) {
                self.undo(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift));
            }
            if ctrl && is_key_pressed(KeyCode::Y) {
                self.undo(true);
            }
            if ctrl && is_key_pressed(KeyCode::S) {
                self.export();
            }
            if is_key_pressed(KeyCode::Home) {
                self.fit();
            }
        }
        // A dropdown owns the click even when that click dismisses it. Nothing
        // behind it should select, drag, delete or launch an object.
        let offscreen = vec2(-100.0, -100.0);
        draw_line(24.0, 79.0, size.x - 24.0, 79.0, 1.0, RULE);
        draw_rectangle(24.0, 22.0, 4.0, 32.0, CYAN);
        text("F R A C T U R E", 42.0, 44.0, 28.0, INK);
        text("OBJECT WORKSHOP", 43.0, 66.0, 14.0, CYAN);
        for (menu, x, label) in [(Menu::File, 390.0, "FILE"), (Menu::View, 500.0, "VIEW")] {
            if button(
                Rect::new(x, 23.0, 96.0, 38.0),
                label,
                mouse,
                self.menu == Some(menu),
            ) && self.commit_input()
            {
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                self.dragging = false;
            }
        }
        let blocked = menu_was_open || self.menu.is_some() || input_blocked;
        let controls_mouse = if blocked { offscreen } else { mouse };
        if button(
            Rect::new(612.0, 23.0, 92.0, 38.0),
            "UNDO",
            controls_mouse,
            false,
        ) {
            self.undo(false);
        }
        if button(
            Rect::new(704.0, 23.0, 92.0, 38.0),
            "REDO",
            controls_mouse,
            false,
        ) {
            self.undo(true);
        }
        let mut action = None;
        if (button(
            Rect::new(size.x - 488.0, 23.0, 140.0, 38.0),
            "O  BACK",
            controls_mouse,
            false,
        ) || (!editing
            && !blocked
            && (is_key_pressed(KeyCode::O) || is_key_pressed(KeyCode::Escape))))
            && self.commit_input()
        {
            action = Some(EditorAction::Close);
        }
        self.refresh();
        let can_test = self.mesh.is_ok();
        if (button(
            Rect::new(size.x - 340.0, 23.0, 148.0, 38.0),
            "ENTER TEST",
            controls_mouse,
            can_test,
        ) || (!editing && !blocked && is_key_pressed(KeyCode::Enter)))
            && self.commit_input()
        {
            match self.asset.mesh() {
                Ok(_) => action = Some(EditorAction::Test(self.asset.clone())),
                Err(e) => self.status = e,
            }
        }
        if button(
            Rect::new(size.x - 184.0, 23.0, 160.0, 38.0),
            "COMPARE",
            controls_mouse,
            can_test,
        ) && self.commit_input()
        {
            match self.asset.mesh() {
                Ok(_) => action = Some(EditorAction::Compare(self.asset.clone())),
                Err(e) => self.status = e,
            }
        }
        text("OBJECT TOOLS", 28.0, 115.0, 21.0, INK);
        for (i, (tab, label)) in [
            (Tab::Shape, "SHAPE"),
            (Tab::Fracture, "FRACTURE"),
            (Tab::Material, "MATERIAL"),
        ]
        .into_iter()
        .enumerate()
        {
            if button(
                Rect::new(20.0 + i as f32 * 116.0, 134.0, 112.0, 38.0),
                label,
                controls_mouse,
                self.tab == tab,
            ) && self.commit_input()
            {
                self.select_tab(tab);
            }
        }
        match self.tab {
            Tab::Shape => self.shape_controls(controls_mouse),
            Tab::Fracture => self.fracture_controls(controls_mouse),
            Tab::Material => self.material_controls(controls_mouse),
        }
        if !editing && !blocked && self.point_editing() && is_key_pressed(KeyCode::Delete) {
            self.delete_vertex();
        }
        draw_line(380.0, 98.0, 380.0, size.y - 101.0, 1.0, RULE);
        text(&self.asset.name, 408.0, 118.0, 24.0, INK);
        right_text(
            if self.asset.ground_anchor {
                "GROUND ANCHORED"
            } else if self.asset.pinned_edges().next().is_some() {
                "PINNED SUPPORT"
            } else {
                "FREE OBJECT"
            },
            size.x - 32.0,
            118.0,
            16.0,
            MUTED,
        );
        let stage = Rect::new(404.0, 145.0, size.x - 432.0, size.y - 290.0);
        self.canvas(
            stage,
            controls_mouse,
            dt,
            pixel_scale,
            editing || blocked || !self.point_editing(),
        );
        if !blocked {
            self.keyboard_input();
        }
        self.refresh();
        if let Ok(mesh) = &self.mesh {
            text(
                &format!(
                    "{} GRAINS / {} CELLS / {:.2} m2 / {:.0} kg",
                    mesh.grains
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    mesh.triangles.len(),
                    mesh.area,
                    mesh.area * self.asset.density * self.asset.thickness
                ),
                408.0,
                size.y - 115.0,
                17.0,
                CYAN,
            );
        } else {
            text("OBJECT NEEDS ATTENTION", 408.0, size.y - 115.0, 18.0, PINK);
        }
        draw_line(24.0, size.y - 94.0, size.x - 24.0, size.y - 94.0, 1.0, RULE);
        wrapped(
            self.mesh
                .as_ref()
                .err()
                .map_or(self.status.as_str(), String::as_str),
            28.0,
            size.y - 66.0,
            size.x - 56.0,
            18.0,
            if self.mesh.is_err() { PINK } else { INK },
        );
        text(
            "WHEEL ZOOM / RIGHT-DRAG PAN / HOME FIT",
            28.0,
            size.y - 18.0,
            15.0,
            MUTED,
        );
        right_text(
            if self.mesh.is_err() {
                "INVALID DRAFT / TEST DISABLED"
            } else if self.saved.as_ref() == Some(&self.asset) {
                "DRAFT SAVED LOCALLY"
            } else {
                "DRAFT NOT SAVED"
            },
            size.x - 28.0,
            size.y - 18.0,
            15.0,
            if self.mesh.is_ok() { MUTED } else { PINK },
        );
        self.dropdown(mouse);
        #[cfg(target_arch = "wasm32")]
        publish(self);
        self.last_mouse = mouse;
        action
    }
}
