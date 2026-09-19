//! Shape, fracture and material sidebar controls.
use super::*;

impl Editor {
    pub(super) fn select_tab(&mut self, tab: Tab) {
        let grain_edit = tab == Tab::Fracture
            && self
                .asset
                .layout
                .as_ref()
                .is_some_and(|l| l.mode == GrainMode::Stones);
        if self.grain_edit != grain_edit {
            self.selected = 0;
        }
        self.tab = tab;
        self.grain_edit = grain_edit;
        self.dragging = false;
    }
    pub(super) fn point_editing(&self) -> bool {
        self.tab == Tab::Shape || (self.tab == Tab::Fracture && self.grain_edit)
    }
    pub(super) fn point_controls(&mut self, y: f32, mouse: Vec2) {
        let count = if self.grain_edit {
            self.asset.layout.as_ref().map_or(0, |l| l.sites.len())
        } else {
            self.asset.outline.len()
        };
        text(
            &format!(
                "{} {:02} / {:02}",
                if self.grain_edit {
                    "GRAIN POINT"
                } else {
                    "VERTEX"
                },
                self.selected + 1,
                count
            ),
            28.0,
            y,
            18.0,
            CYAN,
        );
        self.field(Field::X, "X  m", y + 18.0, mouse);
        self.field(Field::Y, "Y  m", y + 56.0, mouse);
        if button(
            Rect::new(20.0, y + 110.0, 166.0, 37.0),
            "+ ADD",
            mouse,
            false,
        ) {
            if self.grain_edit {
                self.add_grain(Vec2::from(self.selected_point()) + vec2(0.15, 0.15));
            } else {
                let a = Vec2::from(self.asset.outline[self.selected]);
                let b =
                    Vec2::from(self.asset.outline[(self.selected + 1) % self.asset.outline.len()]);
                self.insert_vertex(self.selected, (a + b) * 0.5);
            }
        }
        if button(
            Rect::new(194.0, y + 110.0, 166.0, 37.0),
            "REMOVE",
            mouse,
            false,
        ) {
            self.delete_vertex();
        }
    }
    pub(super) fn shape_controls(&mut self, mouse: Vec2) {
        self.field(Field::Name, "NAME", 202.0, mouse);
        self.point_controls(276.0, mouse);
        if button(
            Rect::new(20.0, 444.0, 340.0, 38.0),
            if self.asset.pinned_edges().any(|e| e == self.selected) {
                "UNPIN SELECTED EDGE"
            } else {
                "PIN SELECTED EDGE"
            },
            mouse,
            self.asset.pinned_edges().any(|e| e == self.selected),
        ) {
            self.checkpoint();
            self.asset.toggle_pin(self.selected);
        }
        wrapped(
            "Drag corners to reshape. Shift-click an edge to add.",
            28.0,
            558.0,
            322.0,
            17.0,
            MUTED,
        );
        if button(
            Rect::new(20.0, 492.0, 340.0, 38.0),
            if self.asset.ground_anchor {
                "GROUND ANCHOR: ON"
            } else {
                "GROUND ANCHOR: OFF"
            },
            mouse,
            self.asset.ground_anchor,
        ) {
            self.checkpoint();
            self.asset.ground_anchor = !self.asset.ground_anchor;
        }
        wrapped(
            "Pin the highlighted edge to add a breakable support.",
            28.0,
            604.0,
            322.0,
            17.0,
            MUTED,
        );
        self.field(Field::Color, "COLOR  hex", 654.0, mouse);
        text("L  OBJECT LIBRARY / FILE", 28.0, 720.0, 16.0, MUTED);
    }
    pub(super) fn fracture_controls(&mut self, mouse: Vec2) {
        text("GRAIN LAYOUT", 28.0, 215.0, 16.0, MUTED);
        for (i, (label, mode)) in [
            ("STONES", Some(GrainMode::Stones)),
            ("BRICKS", Some(GrainMode::Bricks)),
            ("LEGACY", None),
        ]
        .into_iter()
        .enumerate()
        {
            if button(
                Rect::new(20.0 + i as f32 * 116.0, 230.0, 112.0, 36.0),
                label,
                mouse,
                self.asset.layout.as_ref().map(|l| l.mode) == mode,
            ) {
                self.checkpoint();
                let old = self.asset.layout.clone();
                if let Some(mode) = mode {
                    self.asset.refinement = 0;
                    self.regenerate(
                        mode,
                        old.as_ref().map_or([0.7, 0.5], |l| l.size),
                        old.map_or(7, |l| l.seed),
                    );
                } else {
                    self.asset.layout = None;
                    self.asset.refinement = 2;
                }
                self.select_tab(Tab::Fracture);
            }
        }
        if self.asset.layout.is_some() {
            self.field(Field::GrainWidth, "WIDTH  m", 284.0, mouse);
            self.field(Field::GrainHeight, "HEIGHT  m", 322.0, mouse);
            if self.grain_edit {
                self.field(Field::Seed, "SEED", 360.0, mouse);
                if button(
                    Rect::new(20.0, 405.0, 340.0, 36.0),
                    "REGENERATE POINTS",
                    mouse,
                    false,
                ) {
                    self.checkpoint();
                    let l = self.asset.layout.as_ref().unwrap().clone();
                    self.regenerate(l.mode, l.size, l.seed);
                }
                self.point_controls(476.0, mouse);
            } else {
                wrapped(
                    "Rows fit the outline. Set brick size with width and height.",
                    28.0,
                    406.0,
                    322.0,
                    17.0,
                    MUTED,
                );
            }
        } else {
            wrapped(
                "Triangular grains. Try stones or bricks for larger chunks.",
                28.0,
                311.0,
                322.0,
                17.0,
                MUTED,
            );
        }
        self.field(Field::Cohesion, "GRAIN COHESION", 640.0, mouse);
        if button(
            Rect::new(20.0, 694.0, 340.0, 36.0),
            if self.detail_open {
                "- SOLVER DETAIL"
            } else {
                "+ SOLVER DETAIL"
            },
            mouse,
            self.detail_open,
        ) {
            self.detail_open = !self.detail_open;
        }
        if self.detail_open {
            text("DETAIL", 28.0, 764.0, 16.0, MUTED);
            text(&self.asset.refinement.to_string(), 278.0, 766.0, 20.0, INK);
            for (x, delta, label) in [(214.0, -1, "-"), (310.0, 1, "+")] {
                if button(Rect::new(x, 742.0, 44.0, 34.0), label, mouse, false)
                    && (0..=4).contains(&(self.asset.refinement as i32 + delta))
                {
                    self.checkpoint();
                    self.asset.refinement = (self.asset.refinement as i32 + delta) as u8;
                }
            }
        } else {
            text(
                if self.grain_edit {
                    "Drag points / Shift-click to add"
                } else {
                    "Compact grains / rigid chunks"
                },
                28.0,
                766.0,
                16.0,
                MUTED,
            );
        }
    }
    pub(super) fn material_controls(&mut self, mouse: Vec2) {
        if self.bounce_open {
            self.bounce_controls(mouse);
            return;
        }
        if self.behavior_open {
            self.behavior_controls(mouse);
            return;
        }
        if button(
            Rect::new(20.0, 722.0, 224.0, 36.0),
            "OBJECT BEHAVIOR  >",
            mouse,
            false,
        ) {
            self.behavior_open = true;
        }
        if button(
            Rect::new(248.0, 722.0, 112.0, 36.0),
            "BOUNCE >",
            mouse,
            false,
        ) {
            self.bounce_open = true;
        }
        text("MATERIAL PRESET", 28.0, 215.0, 16.0, MUTED);
        for (i, surface) in [Surface::Rock, Surface::Masonry, Surface::Panel]
            .into_iter()
            .enumerate()
        {
            if button(
                Rect::new(20.0 + i as f32 * 116.0, 230.0, 112.0, 36.0),
                surface.label(),
                mouse,
                self.asset.surface == surface,
            ) {
                self.checkpoint();
                self.asset.apply_surface(surface);
            }
        }
        for (i, (field, label)) in [
            (Field::Density, "DENSITY kg/m3"),
            (Field::Thickness, "THICKNESS m"),
            (Field::Friction, "FRICTION"),
            (
                Field::Restitution,
                if self.asset.soft_bounce.is_some() {
                    "BASE BOUNCE"
                } else {
                    "BOUNCINESS 0-1"
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            self.field(field, label, 294.0 + i as f32 * 36.0, mouse);
        }
        if button(
            Rect::new(20.0, 438.0, 340.0, 38.0),
            if self.asset.components.plasticity.is_some() {
                "YIELD & HARDENING  >"
            } else if self.material_open {
                "- FAILURE & STIFFNESS"
            } else {
                "+ FAILURE & STIFFNESS"
            },
            mouse,
            self.material_open,
        ) {
            if self.asset.components.plasticity.is_some() {
                self.behavior_open = true;
                self.plastic_open = true;
            } else {
                self.material_open = !self.material_open;
            }
        }
        if self.asset.components.plasticity.is_some() {
            self.field(Field::Young, "YOUNG E GPa", 497.0, mouse);
            wrapped(
                "Deformable frame: tune yield and hardening in Object Behavior. Seams stay connected.",
                28.0,
                564.0,
                322.0,
                17.0,
                MUTED,
            );
        } else if self.material_open {
            for (i, (field, label)) in [
                (Field::Tension, "TENSION kPa"),
                (Field::Compression, "COMPRESSION kPa"),
                (Field::Shear, "SHEAR kPa"),
                (Field::Young, "YOUNG E GPa"),
                (Field::Toughness, "Gc J/m2"),
            ]
            .into_iter()
            .enumerate()
            {
                self.field(field, label, 497.0 + i as f32 * 42.0, mouse);
            }
        } else {
            wrapped(
                "Density and thickness set mass. Expand to tune failure.",
                28.0,
                522.0,
                322.0,
                17.0,
                MUTED,
            );
        }
    }
    pub(super) fn bounce_controls(&mut self, mouse: Vec2) {
        if button(
            Rect::new(20.0, 194.0, 340.0, 36.0),
            "< MATERIAL PROPERTIES",
            mouse,
            false,
        ) {
            self.bounce_open = false;
        }
        self.field(Field::Friction, "FRICTION", 260.0, mouse);
        self.field(Field::Restitution, "BASE BOUNCE", 302.0, mouse);
        let enabled = self.asset.soft_bounce.is_some();
        if button(
            Rect::new(20.0, 356.0, 340.0, 36.0),
            if enabled {
                "SOFT IMPACT BOUNCE: ON"
            } else {
                "SOFT IMPACT BOUNCE: OFF"
            },
            mouse,
            enabled,
        ) {
            self.checkpoint();
            self.asset.soft_bounce = if enabled {
                None
            } else {
                Some(sim::SoftBounce::default())
            };
        }
        if enabled {
            self.field(Field::SoftBounce, "SOFT BOUNCE", 402.0, mouse);
            self.field(Field::BounceSpeed, "SOFT UP TO m/s", 444.0, mouse);
        }
        for (line, label) in [
            "Soft bounce below chosen speed.",
            "Base bounce at twice the speed.",
            "Smooth transition in between.",
            "Friction controls sliding.",
        ]
        .iter()
        .enumerate()
        {
            text(label, 28.0, 522.0 + line as f32 * 22.0, 16.0, MUTED);
        }
    }
    pub(super) fn behavior_controls(&mut self, mouse: Vec2) {
        if self.plastic_open {
            self.plastic_controls(mouse);
            return;
        }
        if button(
            Rect::new(20.0, 194.0, 340.0, 36.0),
            "< MATERIAL PROPERTIES",
            mouse,
            false,
        ) {
            self.behavior_open = false;
        }
        self.field(Field::ProjectileResistance, "SHELL RESIST x", 260.0, mouse);
        if self.asset.components.plasticity.is_none() {
            self.field(Field::ContactResistance, "CONTACT RESIST x", 302.0, mouse);
        } else {
            text("DEFORMABLE FRAME", 28.0, 326.0, 16.0, MUTED);
        }
        let crush = self.asset.components.crush.is_some();
        if button(
            Rect::new(20.0, 356.0, 340.0, 36.0),
            if crush {
                "CRUSHABLE: ON"
            } else {
                "CRUSHABLE: OFF"
            },
            mouse,
            crush,
        ) {
            self.checkpoint();
            self.asset.components.crush = if crush {
                None
            } else {
                Some(sim::Crushable::default())
            };
        }
        if self.asset.components.crush.is_some() {
            self.field(Field::CrushForce, "CRUSH FORCE kN", 402.0, mouse);
            if self.asset.components.plasticity.is_none() {
                self.field(Field::CrushResistance, "CRUSH SCALE x", 444.0, mouse);
            }
        }
        let explosive = self.asset.components.explosive.is_some();
        if button(
            Rect::new(20.0, 498.0, 340.0, 36.0),
            if explosive {
                "EXPLOSIVE: ON"
            } else {
                "EXPLOSIVE: OFF"
            },
            mouse,
            explosive,
        ) {
            self.checkpoint();
            self.asset.components.explosive = if explosive {
                None
            } else {
                Some(sim::Explosive::default())
            };
        }
        if self.asset.components.explosive.is_some() {
            self.field(Field::ExplosionEnergy, "BLAST ENERGY kJ", 544.0, mouse);
            self.field(Field::ExplosionRadius, "BLAST RADIUS m", 586.0, mouse);
        }
        let refine = self.asset.components.contact_refinement;
        if self.asset.components.plasticity.is_none()
            && button(
                Rect::new(20.0, 646.0, 340.0, 36.0),
                if refine {
                    "CONTACT DETAIL: REFINE"
                } else {
                    "CONTACT DETAIL: KEEP GRAINS"
                },
                mouse,
                refine,
            )
        {
            self.checkpoint();
            self.asset.components.contact_refinement = !refine;
        }
        if button(
            Rect::new(20.0, 718.0, 340.0, 38.0),
            "PERMANENT DEFORMATION  >",
            mouse,
            false,
        ) {
            self.plastic_open = true;
        }
    }
    pub(super) fn plastic_controls(&mut self, mouse: Vec2) {
        if button(
            Rect::new(20.0, 194.0, 340.0, 36.0),
            "< OBJECT BEHAVIOR",
            mouse,
            false,
        ) {
            self.plastic_open = false;
        }
        let enabled = self.asset.components.plasticity.is_some();
        if button(
            Rect::new(20.0, 254.0, 340.0, 38.0),
            if enabled {
                "MODE: DEFORMABLE"
            } else {
                "MODE: DESTRUCTIBLE"
            },
            mouse,
            enabled,
        ) {
            self.checkpoint();
            self.asset.components.plasticity = if enabled {
                None
            } else {
                Some(sim::Plasticity::default())
            };
        }
        if self.asset.components.plasticity.is_some() {
            self.field(Field::YieldStress, "YIELD STRESS kPa", 324.0, mouse);
            self.field(Field::PlasticFlow, "FLOW RATE /s", 376.0, mouse);
            self.field(Field::Hardening, "HARDENING", 428.0, mouse);
            self.field(Field::PlasticLimit, "STRAIN LIMIT", 480.0, mouse);
        }
        wrapped(
            if enabled {
                "Above yield: permanent bends. No fractures or springback."
            } else {
                "Destructible: breaks into chunks. Switch mode for permanent bends."
            },
            28.0,
            573.0,
            322.0,
            16.0,
            MUTED,
        );
        if enabled {
            wrapped(
                "Hardening resists more bending. Strain limit caps permanent change.",
                28.0,
                643.0,
                322.0,
                16.0,
                MUTED,
            );
            wrapped(
                "Crushable needs opposing loads. Turn it off for free bending.",
                28.0,
                713.0,
                322.0,
                16.0,
                MUTED,
            );
        }
    }
}
