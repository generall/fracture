//! Synchronized, recipe-based material trials. Recording belongs to the sim
//! crate; this screen owns playback, authoring and the local experiment library.
use crate::{
    comparison_storage as storage,
    ui::{self, CYAN, INK, MUTED, PAPER, PINK, RULE, VIOLET},
};
use macroquad::prelude::*;
use sim::{
    GrainMode, ObjectAsset,
    experiment::{
        Archive, ENGINE_REVISION, Experiment, FRAME_TICKS, Frame, Geometry, ImpactKind, Runner,
        Sweep, VisualState,
    },
};
use std::collections::BTreeMap;

mod diagnostics;
mod fields;
mod library;
mod panels;
mod screen;
mod view;
use diagnostics::publish;
pub use diagnostics::publish_closed;
use view::{card_view, draw_state, view_scale};

pub enum Action {
    Close,
    Use(ObjectAsset),
    Edit(ObjectAsset),
}
#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    Variant,
    Density,
    Young,
    Toughness,
    Tension,
    Compression,
    Shear,
    Cohesion,
    Friction,
    Restitution,
    SoftBounce,
    BounceSpeed,
    Depth,
    ShellMass,
    ShellSpeed,
    TankMass,
    TankSpeed,
    TankAngle,
    AimX,
    AimY,
    Duration,
    GrainWidth,
    GrainHeight,
    Seed,
    Detail,
    YieldStress,
    PlasticFlow,
    Hardening,
    StrainLimit,
}
struct Input {
    field: Field,
    value: String,
    all: bool,
    rect: Rect,
    shared: bool,
}

pub struct Comparison {
    pub active: bool,
    experiment: Experiment,
    runner: Option<Runner>,
    archive: Archive,
    library_error: Option<String>,
    library_open: bool,
    selected: usize,
    tab: usize,
    bounce_open: bool,
    sweep: usize,
    input: Option<Input>,
    message: String,
    error: Option<String>,
    calculating: bool,
    playing: bool,
    slow: bool,
    looping: bool,
    cursor: f32,
    focus: bool,
    stress: bool,
    center: Vec2,
    zoom: f32,
    target_zoom: f32,
    last_mouse: Vec2,
    zoom_anchor: Option<(Vec2, Vec2, Rect)>,
}
impl Comparison {
    pub fn new(asset: ObjectAsset) -> Self {
        let (archive, library_error) =
            match storage::restore().and_then(|s| s.map(|s| Archive::from_json(&s)).transpose()) {
                Ok(a) => (a.unwrap_or_default(), None),
                Err(e) => (Archive::default(), Some(e)),
            };
        let mut result = Self {
            active: true,
            experiment: Experiment::new(asset),
            runner: None,
            archive,
            library_error,
            library_open: false,
            selected: 1,
            tab: 0,
            bounce_open: false,
            sweep: 0,
            input: None,
            message: "Select a case to tune it. RUN calculates all cases on one timeline.".into(),
            error: None,
            calculating: false,
            playing: false,
            slow: false,
            looping: false,
            cursor: 0.0,
            focus: false,
            stress: false,
            center: vec2(0.5, 0.0),
            zoom: 1.0,
            target_zoom: 1.0,
            last_mouse: Vec2::ZERO,
            zoom_anchor: None,
        };
        result.prepare();
        result
    }
    pub fn barrels() -> Self {
        let mut c = Self::new(sim::LibraryPreset::Barrel.asset());
        c.load(Experiment::barrel_comparison());
        c.looping = true;
        c.run();
        c
    }
    pub fn crush() -> Self {
        let mut c = Self::new(sim::LibraryPreset::Barrel.asset());
        c.load(Experiment::crush_comparison());
        c.looping = true;
        c.run();
        c
    }
    pub fn barrel_shot(backstop: bool) -> Self {
        let mut c = Self::new(sim::LibraryPreset::Barrel.asset());
        c.load(if backstop {
            Experiment::barrel_shot_comparison()
        } else {
            Experiment::barrel_free_shot_comparison()
        });
        c.looping = true;
        c.slow = true;
        c.run();
        c
    }
    pub fn barrel_release() -> Self {
        let mut c = Self::new(sim::LibraryPreset::Barrel.asset());
        c.load(Experiment::barrel_release_comparison());
        c.looping = true;
        c.slow = true;
        c.run();
        c
    }
    fn fit_view(&mut self) {
        let close = self.experiment.variants.iter().all(|v| v.impact.backstop);
        self.center = if close {
            vec2(2.4, 0.0)
        } else {
            vec2(0.5, 0.0)
        };
        self.zoom = if close {
            if self.experiment.variants.iter().any(|v| {
                v.impact.kind == ImpactKind::Tank
                    && (v.impact.aim[1].abs() > 0.1 || v.impact.angle_degrees != 0.0)
            }) {
                2.0 // Leave room for the offset tank and its contact-driven turn.
            } else {
                3.0
            }
        } else {
            1.0
        };
        self.target_zoom = self.zoom;
        self.zoom_anchor = None;
    }
    pub fn selected_object(&self) -> ObjectAsset {
        self.experiment.variants[self.selected].object.clone()
    }
    pub fn replace_selected(&mut self, asset: ObjectAsset) {
        if self.experiment.variants[self.selected].object != asset {
            self.experiment.variants[self.selected].object = asset;
            self.changed();
        }
        self.active = true;
    }
    fn prepare(&mut self) {
        self.calculating = false;
        self.playing = false;
        match Runner::new(self.experiment.clone()) {
            Ok(r) => {
                self.runner = Some(r);
                self.error = None;
            }
            Err(e) => {
                self.runner = None;
                self.error = Some(e);
            }
        }
    }
    fn changed(&mut self) {
        if self.experiment.shared_impact {
            let impact = self.experiment.variants[self.selected].impact.clone();
            for v in &mut self.experiment.variants {
                v.impact = impact.clone();
            }
        }
        self.experiment.invalidate();
        self.cursor = 0.0;
        self.prepare();
        self.message = "Settings changed. RUN to record this experiment.".into();
    }
    fn run(&mut self) {
        // A replay keeps the recipe and favorite but replaces its visual cache.
        self.experiment.review.clear();
        self.experiment.cursor_tick = 0;
        self.experiment.engine = ENGINE_REVISION.into();
        self.cursor = 0.0;
        self.prepare();
        if self.runner.is_some() {
            self.calculating = true;
            self.playing = true;
            self.message =
                "Recording all cases at 120 Hz. Playback waits for the slowest case.".into();
        }
    }
    fn available(&self) -> u32 {
        self.runner
            .as_ref()
            .map_or(self.experiment.cursor_tick, |r| {
                if r.finished() {
                    r.completed_tick
                } else {
                    r.completed_tick / FRAME_TICKS * FRAME_TICKS
                }
            })
    }
    fn state(&self, i: usize) -> Option<VisualState> {
        self.runner
            .as_ref()
            .map(|r| r.recordings[i].state(self.cursor as u32))
            .or_else(|| self.experiment.review.get(i).cloned())
    }
    fn cards(&self, area: Rect) -> Vec<(usize, Rect)> {
        if self.focus {
            vec![(self.selected, area)]
        } else {
            let n = self.experiment.variants.len();
            let columns = if n <= 4 { n.min(2) } else { n.div_ceil(2) };
            let rows = n.div_ceil(columns);
            let width = (area.w - 12.0 * (columns - 1) as f32) / columns as f32;
            let height = (area.h - 12.0 * (rows - 1) as f32) / rows as f32;
            (0..self.experiment.variants.len())
                .map(|i| {
                    (
                        i,
                        Rect::new(
                            area.x + (i % columns) as f32 * (width + 12.0),
                            area.y + (i / columns) as f32 * (height + 12.0),
                            width,
                            height,
                        ),
                    )
                })
                .collect()
        }
    }
}
