//! Map authoring owns recipes, never live physics. Playtesting clones the draft.
mod canvas;
#[cfg(target_arch = "wasm32")]
mod diagnostics;
mod fields;
mod panels;
mod selection;
mod terrain_brush;
use crate::ui::PINK;
use crate::{level_storage as storage, navigation::Navigation, ui::*, view::Viewport};
use macroquad::prelude::*;
use sim::level::{Level, LevelItem, StationKind, StationSettings, Team};
use sim::{LibraryPreset, ObjectAsset, ObjectLibrary, TerrainKind};

pub enum Action {
    Close,
    Play(Box<Level>, Team),
}
#[derive(Clone)]
enum Tool {
    Select,
    Erase,
    Object(Box<ObjectAsset>),
    Wall,
    Fence,
    Solid,
    SolidWall,
    Terrain(TerrainKind),
    Bridge,
    Spawn(Team),
    Flag(Team),
    Station(StationKind),
    Pickup(sim::SpecialWeapon),
}
impl Tool {
    fn draws_cells(&self) -> bool {
        matches!(
            self,
            Self::Wall
                | Self::Fence
                | Self::SolidWall
                | Self::Solid
                | Self::Terrain(_)
                | Self::Bridge
                | Self::Erase
        )
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    MapWidth,
    MapHeight,
    X,
    Y,
    Width,
    Height,
    Angle,
    GrainWidth,
    GrainHeight,
    CaptureTime,
    CaptureRadius,
    RepairRate,
    RadarRadius,
    PickupRespawn,
    PickupCharges,
    PickupCooldown,
}
struct Editing {
    field: Field,
    value: String,
    fresh: bool,
}
#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Vertex(usize, usize),
    Resize(usize),
    Draw,
    Erase,
    Marquee,
    Brush,
}
struct Drag {
    before: Level,
    start: Vec2,
    kind: DragKind,
    selection: Vec<usize>,
}
pub struct LevelEditor {
    pub active: bool,
    level: Level,
    geometry: Option<(Level, Vec<Option<sim::ObjectMesh>>)>,
    terrain_cache: Option<(sim::level::TerrainMask, Vec<sim::TerrainPatch>)>,
    selected: Option<usize>,
    selection: Vec<usize>,
    clipboard: Vec<LevelItem>,
    paste_count: u32,
    join: bool,
    tool: Tool,
    terrain_brush: bool,
    eraser_brush: bool,
    brush_size: i32,
    brush_grid: f32,
    brush_stroke: Option<terrain_brush::Stroke>,
    category: usize,
    palette: Vec<ObjectAsset>,
    page: usize,
    pub team: Team,
    navigation: Navigation,
    last_mouse: Vec2,
    fit_requested: bool,
    drag: Option<Drag>,
    undo: Vec<Level>,
    redo: Vec<Level>,
    editing: Option<Editing>,
    vertices: bool,
    snap: bool,
    file_menu: bool,
    status: String,
    restore_error: Option<String>,
}
fn v(p: sim::Vec2) -> Vec2 {
    vec2(p.x, p.y)
}
fn s(p: Vec2) -> sim::Vec2 {
    sim::Vec2::new(p.x, p.y)
}

impl LevelEditor {
    pub fn new() -> Self {
        let restored =
            storage::restore().and_then(|json| json.map(|j| Level::from_json(&j)).transpose());
        let (mut level, restore_error) = match restored {
            Ok(l) => (l.unwrap_or_default(), None),
            Err(e) => (Level::default(), Some(e)),
        };
        level.migrate_terrain();
        Self {
            active: false,
            level,
            geometry: None,
            terrain_cache: None,
            selected: None,
            selection: vec![],
            clipboard: vec![],
            paste_count: 0,
            join: true,
            tool: Tool::Select,
            terrain_brush: true,
            eraser_brush: true,
            brush_size: 5,
            brush_grid: 0.5,
            brush_stroke: None,
            category: 0,
            palette: vec![],
            page: 0,
            team: Team::Blue,
            navigation: Navigation::default(),
            last_mouse: Vec2::ZERO,
            fit_requested: true,
            drag: None,
            undo: vec![],
            redo: vec![],
            editing: None,
            vertices: false,
            snap: false,
            file_menu: false,
            status: "Choose a tool, then place it on the map. Select to move or reshape.".into(),
            restore_error,
        }
    }
    pub fn open(&mut self) {
        self.active = true;
        self.drag = None;
        self.editing = None;
        self.palette = LibraryPreset::ALL.into_iter().map(|p| p.asset()).collect();
        match crate::editor_storage::restore_library()
            .and_then(|j| j.map(|j| ObjectLibrary::from_json(&j)).transpose())
        {
            Ok(Some(l)) => self.palette.extend(l.objects),
            Ok(None) => {}
            Err(e) => self.status = format!("Object library: {e}"),
        }
    }
    fn persist(&mut self) {
        if let Some(e) = &self.restore_error {
            self.status = format!("Saved level could not be read: {e} Export to keep this draft.");
            return;
        }
        match self.level.to_json().and_then(|j| storage::persist(&j)) {
            Ok(()) => self.status = "Draft saved on this device.".into(),
            Err(e) => self.status = e,
        }
    }
    fn save_file(&mut self) {
        self.commit_field();
        self.status = self
            .level
            .to_json()
            .and_then(|j| storage::export(&j))
            .map(str::to_string)
            .unwrap_or_else(|e| e);
    }
    fn open_file(&mut self) {
        self.commit_field();
        if let Err(error) = storage::pick_import() {
            self.status = error;
        }
    }
    fn finish_change(&mut self, before: Level) {
        if self.level.migrate_terrain() > 0 {
            self.clear_selection();
        }
        self.level.terrain_mask.crop(self.level.size.into());
        if self.level == before {
            return;
        }
        if let Err(e) = self
            .level
            .validate()
            .and_then(|()| self.level.resolved_meshes().map(|_| ()))
        {
            self.level = before;
            self.status = e;
            self.clamp_selection();
            return;
        }
        self.undo.push(before);
        if self.undo.len() > 32 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.persist();
        self.clamp_selection();
    }
    fn clamp_selection(&mut self) {
        self.selection.retain(|&i| i < self.level.items.len());
        self.selected = self.selection.last().copied();
    }
    fn history(&mut self, redo: bool) {
        self.editing = None;
        self.drag = None;
        let next = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(next) = next {
            let old = std::mem::replace(&mut self.level, next);
            if redo {
                self.undo.push(old);
            } else {
                self.redo.push(old);
            }
            self.clear_selection();
            self.persist();
        }
    }
    fn uses_brush(&self) -> bool {
        match self.tool {
            Tool::Terrain(_) => self.terrain_brush,
            Tool::Erase => self.eraser_brush,
            _ => false,
        }
    }
    fn stroke(&self) -> terrain_brush::Stroke {
        terrain_brush::Stroke::new(
            match self.tool {
                Tool::Terrain(terrain) => terrain,
                _ => TerrainKind::Ground,
            },
            self.brush_grid,
        )
    }
    fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.clear_selection();
        self.editing = None;
        self.vertices = false;
        self.status = match self.tool {
            Tool::Select => {
                "Shift-click adds to selection. Drag empty space to box-select. Corners resize."
                    .into()
            }
            Tool::Bridge => {
                "Drag to fill 1 m grid cells. Bridges need at least 3 x 2 cells plus room for booths."
                    .into()
            }
            Tool::Erase if self.eraser_brush => "Drag to brush away objects, walls and terrain. Ctrl+Z undoes a stroke; Esc cancels.".into(),
            Tool::Erase => "Drag to erase a rectangle from objects, walls and terrain. Terrain returns to ground. Ctrl+Z undoes.".into(),
            Tool::Terrain(_) if self.terrain_brush => {
                "Drag to paint terrain along the cursor. Size controls brush width. Ctrl+Z undoes a stroke; Esc cancels.".into()
            }
            Tool::Wall | Tool::Fence | Tool::SolidWall | Tool::Terrain(_) | Tool::Solid => {
                "Click to fill a 1 m grid cell. Drag to fill a rectangle of cells.".into()
            }
            _ => "Click on the map to place. Esc returns to selection.".into(),
        };
    }
    pub fn draw(&mut self, size: Vec2, mouse: Vec2, dt: f32, pixel_scale: f32) -> Option<Action> {
        if let Some(result) = storage::take_import() {
            match result.and_then(|j| Level::from_json(&j)) {
                Ok(level) => {
                    self.drag = None;
                    self.editing = None;
                    let before = std::mem::replace(&mut self.level, level);
                    self.clear_selection();
                    self.finish_change(before);
                    self.fit_requested = true;
                }
                Err(e) => self.status = e,
            }
        }
        let typing = self.editing.is_some();
        self.keyboard_fields();
        if !typing && self.drag.is_none() {
            let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
            if ctrl && is_key_pressed(KeyCode::Z) {
                self.history(false);
            }
            if ctrl && is_key_pressed(KeyCode::Y) {
                self.history(true);
            }
            if ctrl && is_key_pressed(KeyCode::A) {
                self.selection = self
                    .level
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        self.category == 1 || !matches!(item, LevelItem::Terrain { .. })
                    })
                    .map(|(i, _)| i)
                    .collect();
                self.clamp_selection();
                self.tool = Tool::Select;
            }
            if ctrl && is_key_pressed(KeyCode::C) {
                self.copy_selection();
            }
            if ctrl && is_key_pressed(KeyCode::X) {
                self.copy_selection();
                self.delete();
            }
            if ctrl && is_key_pressed(KeyCode::V) {
                self.paste();
            }
            if ctrl && is_key_pressed(KeyCode::D) {
                self.copy_selection();
                self.paste();
            }
            if ctrl && is_key_pressed(KeyCode::J) {
                self.merge_selection();
            }
            if ctrl && is_key_pressed(KeyCode::S) {
                self.save_file();
            }
            if ctrl && is_key_pressed(KeyCode::O) {
                self.open_file();
            }
            if is_key_pressed(KeyCode::Escape) {
                self.select_tool(Tool::Select);
                self.file_menu = false;
            }
            if is_key_pressed(KeyCode::Delete) {
                self.delete();
            }
            if is_key_pressed(KeyCode::Home) {
                self.fit_requested = true;
            }
            if is_key_pressed(KeyCode::M) {
                return Some(Action::Close);
            }
        }
        let canvas = Rect::new(292.0, 88.0, size.x - 588.0, size.y - 174.0);
        if typing && canvas.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
            self.commit_field();
        }
        self.draw_canvas(canvas, mouse, dt, pixel_scale, !typing && !self.file_menu);
        // Panels mask the canvas and own all clicks outside it.
        draw_rectangle(0.0, 0.0, size.x, 80.0, PAPER);
        text("FRACTURE / LEVEL WORKSHOP", 24.0, 35.0, 24.0, INK);
        text(&self.level.name, 24.0, 62.0, 16.0, CYAN);
        if button(
            Rect::new(450.0, 23.0, 85.0, 38.0),
            "FILE",
            mouse,
            self.file_menu,
        ) {
            self.commit_field();
            self.file_menu = !self.file_menu;
        }
        if button(Rect::new(545.0, 23.0, 90.0, 38.0), "UNDO", mouse, false) {
            self.history(false);
        }
        if button(Rect::new(645.0, 23.0, 90.0, 38.0), "REDO", mouse, false) {
            self.history(true);
        }
        if button(
            Rect::new(size.x - 560.0, 23.0, 150.0, 38.0),
            &format!("TEAM {}", self.team.label()),
            mouse,
            false,
        ) {
            self.team = self.team.other();
        }
        if button(
            Rect::new(size.x - 400.0, 23.0, 200.0, 38.0),
            "PLAY LEVEL",
            mouse,
            false,
        ) {
            self.commit_field();
            match sim::World::from_level(&self.level, self.team) {
                Ok(_) => {
                    self.persist();
                    return Some(Action::Play(Box::new(self.level.clone()), self.team));
                }
                Err(e) => self.status = e,
            }
        }
        if button(
            Rect::new(size.x - 190.0, 23.0, 166.0, 38.0),
            "M  BACK",
            mouse,
            false,
        ) {
            self.commit_field();
            return Some(Action::Close);
        }
        self.palette_panel(size, mouse);
        self.inspector(size, mouse);
        draw_rectangle(0.0, size.y - 80.0, size.x, 80.0, PAPER);
        text(&self.status, 24.0, size.y - 51.0, 16.0, INK);
        text(
            "SHIFT SELECT / CTRL C V COPY PASTE / CTRL J MERGE / DEL REMOVE / CTRL Z UNDO / HOME FIT",
            24.0,
            size.y - 22.0,
            14.0,
            MUTED,
        );
        if self.file_menu {
            self.file_panel(mouse);
        }
        #[cfg(target_arch = "wasm32")]
        diagnostics::publish(self, canvas);
        self.last_mouse = mouse;
        None
    }
    fn file_panel(&mut self, mouse: Vec2) {
        let rect = Rect::new(450.0, 70.0, 260.0, 334.0);
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, CYAN);
        for (i, label) in [
            "SAVE AS... / CTRL S",
            "OPEN... / CTRL O",
            "SAVE DRAFT",
            "RELOAD DRAFT",
            "NEW EMPTY LEVEL",
            "LOAD DEMO / QUARRY",
            "LOAD RIVER VALLEY",
        ]
        .iter()
        .enumerate()
        {
            if button(
                Rect::new(rect.x + 8.0, rect.y + 8.0 + i as f32 * 46.0, 244.0, 38.0),
                label,
                mouse,
                false,
            ) {
                self.file_menu = false;
                match i {
                    0 => self.save_file(),
                    1 => self.open_file(),
                    2 => self.persist(),
                    3 => match storage::restore()
                        .and_then(|j| j.ok_or("No saved level.".into()))
                        .and_then(|j| Level::from_json(&j))
                    {
                        Ok(level) => {
                            let before = std::mem::replace(&mut self.level, level);
                            self.finish_change(before);
                            self.clear_selection();
                            self.fit_requested = true;
                        }
                        Err(e) => self.status = e,
                    },
                    4 => {
                        let before = std::mem::take(&mut self.level);
                        self.finish_change(before);
                        self.clear_selection();
                        self.fit_requested = true;
                    }
                    _ => {
                        let map = if i == 5 {
                            Level::demo()
                        } else {
                            Level::river_valley()
                        };
                        let before = std::mem::replace(&mut self.level, map);
                        self.finish_change(before);
                        self.clear_selection();
                        self.tool = Tool::Select;
                        self.vertices = false;
                        self.fit_requested = true;
                        if self.restore_error.is_none() {
                            self.status = if i == 5 { "Quarry Crossing / bridge rush, sandy north bypass, southern road. Choose a team and Play Level." } else { "River Valley / village, radar island and grassy flanks. Choose a team and Play Level." }.into();
                        }
                    }
                }
            }
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub fn publish_closed() {
    diagnostics::closed();
}
#[cfg(not(target_arch = "wasm32"))]
pub fn publish_closed() {}
