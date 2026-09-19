//! Shared test controls and focused material/impact/mesh panels.
use super::*;

impl Comparison {
    pub(super) fn contact_offsets(&self) -> [f32; 3] {
        let v = &self.experiment.variants[self.selected];
        let angle = v.impact.angle_degrees.to_radians();
        let side = vec2(-angle.sin(), angle.cos());
        let min = v
            .object
            .outline
            .iter()
            .map(|p| Vec2::from(*p).dot(side))
            .fold(f32::INFINITY, f32::min);
        let max = v
            .object
            .outline
            .iter()
            .map(|p| Vec2::from(*p).dot(side))
            .fold(f32::NEG_INFINITY, f32::max);
        let middle = (min + max) * 0.5;
        [middle, middle - 0.82, middle + 0.82]
    }
    pub(super) fn shared_test_bar(&mut self, mouse: Vec2, ui_mouse: Vec2) {
        let mouse = if self.library_open {
            vec2(-100.0, -100.0)
        } else {
            mouse
        };
        ui::text("TEST ALL CASES", 28.0, 168.0, 15.0, CYAN);
        let tank = self.experiment.variants[self.selected].impact.kind == ImpactKind::Tank;
        if ui::button(
            Rect::new(28.0, 175.0, 160.0, 34.0),
            if tank { "TANK RAM >" } else { "SHELL >" },
            ui_mouse,
            false,
        ) {
            self.experiment.variants[self.selected].impact.kind = if tank {
                ImpactKind::Shell
            } else {
                ImpactKind::Tank
            };
            self.experiment.shared_impact = true;
            self.changed();
        }
        let fields = [
            (
                if tank {
                    Field::TankSpeed
                } else {
                    Field::ShellSpeed
                },
                "SPEED m/s",
            ),
            (
                if tank { Field::TankAngle } else { Field::AimX },
                if tank { "ANGLE deg" } else { "TARGET X m" },
            ),
            (Field::AimY, if tank { "OFFSET m" } else { "TARGET Y m" }),
            (
                if tank {
                    Field::TankMass
                } else {
                    Field::ShellMass
                },
                "MASS kg",
            ),
            (Field::Duration, "DURATION s"),
        ];
        for (j, (field, label)) in fields.into_iter().enumerate() {
            if !tank
                && self.experiment.variants[self.selected].impact.ammunition
                    == sim::Ammunition::Laser
                && matches!(field, Field::ShellMass | Field::ShellSpeed)
            {
                continue;
            }
            self.field_scoped(
                field,
                label,
                Rect::new(206.0 + j as f32 * 138.0, 175.0, 120.0, 34.0),
                mouse,
                true,
            );
        }
        if tank {
            let offsets = self.contact_offsets();
            let current = offsets.iter().position(|v| {
                (*v - self.experiment.variants[self.selected].impact.aim[1]).abs() < 0.001
            });
            ui::text("TRACK OVERLAP", 896.0, 168.0, 15.0, MUTED);
            if ui::button(
                Rect::new(896.0, 175.0, 206.0, 34.0),
                current.map_or("CUSTOM >", |i| {
                    ["CENTER >", "UPPER HALF >", "LOWER HALF >"][i]
                }),
                ui_mouse,
                false,
            ) {
                self.experiment.variants[self.selected].impact.aim[1] =
                    offsets[current.map_or(0, |i| (i + 1) % 3)];
                self.experiment.shared_impact = true;
                self.changed();
            }
        } else {
            let ammo = self.experiment.variants[self.selected].impact.ammunition;
            ui::text("AMMUNITION", 896.0, 168.0, 15.0, MUTED);
            if ui::button(
                Rect::new(896.0, 175.0, 206.0, 34.0),
                &format!("{} >", ammo.label()),
                ui_mouse,
                false,
            ) {
                self.experiment.variants[self.selected]
                    .impact
                    .select_ammunition(ammo.next());
                self.experiment.shared_impact = true;
                self.changed();
            }
        }
        if ui::button(
            Rect::new(1118.0, 175.0, 156.0, 34.0),
            "BACKSTOP",
            ui_mouse,
            self.experiment.variants[self.selected].impact.backstop,
        ) {
            self.experiment.variants[self.selected].impact.backstop ^= true;
            self.experiment.shared_impact = true;
            self.changed();
        }
        if ui::button(
            Rect::new(1290.0, 175.0, 126.0, 34.0),
            "RUN ALL",
            ui_mouse,
            false,
        ) {
            if !self.experiment.shared_impact {
                self.experiment.shared_impact = true;
                self.changed();
            }
            self.fit_view();
            self.run();
        }
        draw_line(24.0, 221.0, 1416.0, 221.0, 1.0, RULE);
    }
    pub(super) fn sidebar(&mut self, x: f32, mouse: Vec2, ui_mouse: Vec2) -> Option<Action> {
        draw_line(x - 12.0, 96.0, x - 12.0, screen_height(), 1.0, RULE);
        let mouse = if self.library_open {
            vec2(-100.0, -100.0)
        } else {
            mouse
        };
        self.field(
            Field::Variant,
            "SELECTED CASE",
            Rect::new(x, 111.0, 344.0, 33.0),
            mouse,
        );
        for (i, label) in ["MATERIAL", "IMPACT", "MESH"].into_iter().enumerate() {
            if ui::button(
                Rect::new(x + i as f32 * 115.0, 156.0, 111.0, 35.0),
                label,
                ui_mouse,
                self.tab == i,
            ) {
                self.tab = i;
            }
        }
        let fields: &[(Field, &str)] = match self.tab {
            0 if self.bounce_open => &[
                (Field::Friction, "FRICTION"),
                (Field::Restitution, "BASE BOUNCE"),
                (Field::SoftBounce, "SOFT BOUNCE"),
                (Field::BounceSpeed, "SOFT UP TO m/s"),
            ],
            0 if self.experiment.variants[self.selected]
                .object
                .components
                .plasticity
                .is_some() =>
            {
                &[
                    (Field::Density, "DENSITY kg/m3"),
                    (Field::Young, "YOUNG E GPa"),
                    (Field::YieldStress, "YIELD STRESS kPa"),
                    (Field::PlasticFlow, "FLOW RATE /s"),
                    (Field::Hardening, "HARDENING"),
                    (Field::StrainLimit, "STRAIN LIMIT"),
                    (Field::Friction, "FRICTION"),
                    (Field::Depth, "DEPTH m"),
                    (Field::Restitution, "BOUNCINESS 0-1"),
                ]
            }
            0 => &[
                (Field::Density, "DENSITY kg/m3"),
                (Field::Young, "YOUNG E GPa"),
                (Field::Toughness, "FRACTURE Gc J/m2"),
                (Field::Tension, "TENSION kPa"),
                (Field::Compression, "COMPRESSION kPa"),
                (Field::Shear, "SHEAR kPa"),
                (Field::Cohesion, "GRAIN COHESION"),
                (Field::Friction, "FRICTION"),
                (Field::Depth, "DEPTH m"),
                (Field::Restitution, "BOUNCINESS 0-1"),
            ],
            1 if self.experiment.variants[self.selected].impact.kind == ImpactKind::Tank => &[
                (Field::TankMass, "TANK kg"),
                (Field::TankSpeed, "TANK m/s"),
                (Field::AimY, "LATERAL OFFSET m"),
                (Field::Duration, "DURATION s"),
                (Field::TankAngle, "APPROACH deg"),
            ],
            1 if self.experiment.variants[self.selected].impact.ammunition
                == sim::Ammunition::Laser =>
            {
                &[
                    (Field::AimX, "AIM X m"),
                    (Field::AimY, "AIM Y m"),
                    (Field::Duration, "DURATION s"),
                ]
            }
            1 => &[
                (Field::ShellMass, "SHELL kg"),
                (Field::ShellSpeed, "SHELL m/s"),
                (Field::TankMass, "TANK kg"),
                (Field::TankSpeed, "TANK m/s"),
                (Field::AimX, "AIM X m"),
                (Field::AimY, "AIM Y m"),
                (Field::Duration, "DURATION s"),
            ],
            _ => &[
                (Field::GrainWidth, "GRAIN WIDTH m"),
                (Field::GrainHeight, "GRAIN HEIGHT m"),
                (Field::Seed, "LAYOUT SEED"),
                (Field::Detail, "SOLVER DETAIL"),
            ],
        };
        if self.tab == 1 {
            for (j, kind) in [ImpactKind::Shell, ImpactKind::Tank]
                .into_iter()
                .enumerate()
            {
                if ui::button(
                    Rect::new(x + j as f32 * 174.0, 202.0, 168.0, 35.0),
                    &if j == 0 {
                        format!(
                            "AMMO {} >",
                            self.experiment.variants[self.selected]
                                .impact
                                .ammunition
                                .label()
                        )
                    } else {
                        "TANK RAM".into()
                    },
                    ui_mouse,
                    self.experiment.variants[self.selected].impact.kind == kind,
                ) {
                    let impact = &mut self.experiment.variants[self.selected].impact;
                    if kind == ImpactKind::Shell && impact.kind == kind {
                        impact.select_ammunition(impact.ammunition.next());
                    }
                    impact.kind = kind;
                    self.changed();
                }
            }
        }
        if self.tab == 2 {
            for (j, mode) in [None, Some(GrainMode::Stones), Some(GrainMode::Bricks)]
                .into_iter()
                .enumerate()
            {
                let a = &self.experiment.variants[self.selected].object;
                if ui::button(
                    Rect::new(x + j as f32 * 115.0, 202.0, 111.0, 35.0),
                    ["OLD MESH", "STONES", "BRICKS"][j],
                    ui_mouse,
                    a.layout.as_ref().map(|l| l.mode) == mode,
                ) {
                    let a = &mut self.experiment.variants[self.selected].object;
                    if let Some(mode) = mode {
                        let l = a.layout.clone();
                        a.refinement = 0;
                        a.regenerate_grains(
                            mode,
                            l.as_ref().map_or([0.7, 0.5], |l| l.size),
                            l.map_or(7, |l| l.seed),
                        );
                    } else {
                        a.layout = None;
                        a.refinement = 2;
                    }
                    self.changed();
                }
            }
        }
        let top = if self.tab == 0 { 223.0 } else { 265.0 };
        for (j, (field, label)) in fields.iter().enumerate() {
            self.field(
                *field,
                label,
                Rect::new(
                    x + (j % 2) as f32 * 174.0,
                    top + (j / 2) as f32 * 61.0,
                    168.0,
                    32.0,
                ),
                mouse,
            );
        }
        if self.tab == 0 {
            if self.bounce_open {
                let enabled = self.experiment.variants[self.selected]
                    .object
                    .soft_bounce
                    .is_some();
                if ui::button(
                    Rect::new(x, 345.0, 344.0, 36.0),
                    if enabled {
                        "SOFT IMPACT BOUNCE: ON"
                    } else {
                        "SOFT IMPACT BOUNCE: OFF"
                    },
                    ui_mouse,
                    enabled,
                ) {
                    self.experiment.variants[self.selected].object.soft_bounce = if enabled {
                        None
                    } else {
                        Some(sim::SoftBounce::default())
                    };
                    self.changed();
                }
            }
            if ui::button(
                Rect::new(x, 513.0, 344.0, 32.0),
                if self.bounce_open {
                    "< MATERIAL PROPERTIES"
                } else {
                    "BOUNCE RESPONSE >"
                },
                ui_mouse,
                false,
            ) {
                self.bounce_open = !self.bounce_open;
            }
            let plastic = self.experiment.variants[self.selected]
                .object
                .components
                .plasticity
                .is_some();
            ui::text(
                if self.bounce_open {
                    "Soft bounce below the chosen speed."
                } else if plastic {
                    "Strain limit caps permanent bend."
                } else {
                    "E controls virtual strain/energy."
                },
                x,
                573.0,
                15.0,
                MUTED,
            );
            ui::text(
                if self.bounce_open {
                    "Base bounce at twice that speed."
                } else if plastic {
                    "Yield sets resistance; flow sets rate."
                } else {
                    "Connected chunks stay rigid."
                },
                x,
                594.0,
                15.0,
                MUTED,
            );
        }
        if self.tab == 1 {
            if self.experiment.variants[self.selected].impact.kind == ImpactKind::Shell {
                let ammo = self.experiment.variants[self.selected].impact.ammunition;
                ui::text(
                    if ammo.automatic() {
                        "Held fire for the trial duration."
                    } else {
                        "One shot at the start of the trial."
                    },
                    x,
                    504.0,
                    15.0,
                    MUTED,
                );
            }
            if self.experiment.variants[self.selected].impact.kind == ImpactKind::Tank {
                let [middle, upper, lower] = self.contact_offsets();
                for (j, (label, offset)) in
                    [("FULL", middle), ("UPPER 1/2", upper), ("LOWER 1/2", lower)]
                        .into_iter()
                        .enumerate()
                {
                    if ui::button(
                        Rect::new(x + j as f32 * 116.0, 416.0, 110.0, 34.0),
                        label,
                        ui_mouse,
                        (self.experiment.variants[self.selected].impact.aim[1] - offset).abs()
                            < 0.001,
                    ) {
                        self.experiment.variants[self.selected].impact.aim[1] = offset;
                        self.changed();
                    }
                }
                ui::text("Offset is across the approach.", x, 467.0, 15.0, MUTED);
                ui::text(
                    if self.experiment.shared_impact {
                        "Test edits apply to ALL cases."
                    } else {
                        "Test edits apply to THIS case."
                    },
                    x,
                    489.0,
                    15.0,
                    MUTED,
                );
            }
            if ui::button(
                Rect::new(x, 558.0, 344.0, 34.0),
                "PIN AGAINST BACKSTOP",
                ui_mouse,
                self.experiment.variants[self.selected].impact.backstop,
            ) {
                self.experiment.variants[self.selected].impact.backstop =
                    !self.experiment.variants[self.selected].impact.backstop;
                self.changed();
            }
            if ui::button(
                Rect::new(x, 515.0, 344.0, 36.0),
                "COPY IMPACT TO ALL CASES",
                ui_mouse,
                false,
            ) {
                let impact = self.experiment.variants[self.selected].impact.clone();
                for v in &mut self.experiment.variants {
                    v.impact = impact.clone();
                }
                self.changed();
            }
        }
        if self.tab == 2 {
            if ui::button(
                Rect::new(x, 400.0, 344.0, 36.0),
                "EDIT SHAPE / GRAIN POINTS",
                ui_mouse,
                false,
            ) {
                return Some(Action::Edit(
                    self.experiment.variants[self.selected].object.clone(),
                ));
            }
            ui::text("Move stone points in the editor.", x, 469.0, 15.0, MUTED);
            ui::text("COMPARE returns to this case.", x, 493.0, 15.0, MUTED);
            ui::text("Detail subdivides solver cells;", x, 539.0, 15.0, MUTED);
            ui::text("grain size sets fracture layout.", x, 563.0, 15.0, MUTED);
        }
        draw_line(x, 600.0, x + 344.0, 600.0, 1.0, RULE);
        ui::text("SWEEP FROM SELECTED CASE", x, 625.0, 16.0, INK);
        let sweeps: &[Sweep] = if self.experiment.variants[self.selected]
            .object
            .components
            .plasticity
            .is_some()
        {
            &Sweep::PLASTIC
        } else {
            &Sweep::ALL
        };
        let sweeps: Vec<_> = sweeps
            .iter()
            .copied()
            .filter(|s| {
                !self.experiment.shared_impact
                    || !matches!(
                        s,
                        Sweep::ShellMass | Sweep::ShellSpeed | Sweep::TankMass | Sweep::TankSpeed
                    )
            })
            .collect();
        self.sweep %= sweeps.len();
        if ui::button(
            Rect::new(x, 637.0, 344.0, 37.0),
            &format!("{}  >", sweeps[self.sweep].label()),
            ui_mouse,
            false,
        ) {
            self.sweep = (self.sweep + 1) % sweeps.len();
        }
        if ui::button(
            Rect::new(x, 682.0, 344.0, 37.0),
            if self.experiment.variants.len() == 6 {
                "MAKE 6 VARIANTS"
            } else {
                "MAKE 4 VARIANTS"
            },
            ui_mouse,
            false,
        ) {
            let count = if self.experiment.variants.len() == 6 {
                6
            } else {
                4
            };
            self.experiment
                .sweep_count(self.selected, sweeps[self.sweep], count);
            self.selected = if count == 6 { 2 } else { 1 };
            self.cursor = 0.0;
            self.prepare();
            self.message = format!("Created {count} object variants. RUN to compare.");
        }
        ui::text(
            if self.experiment.variants.len() == 6 {
                "0.33x / 0.67x / 1x / 1.33x / 1.83x / 2.5x"
            } else {
                "0.5x / 1x / 1.5x / 2x (bounded)"
            },
            x,
            741.0,
            if self.experiment.variants.len() == 6 {
                12.0
            } else {
                15.0
            },
            MUTED,
        );
        if ui::button(
            Rect::new(x, 760.0, 344.0, 37.0),
            "MARK FAVORITE",
            ui_mouse,
            self.experiment.favorite == Some(self.selected),
        ) {
            self.experiment.favorite = Some(self.selected);
            self.message = "Favorite marked. SAVE to keep it with this experiment.".into();
        }
        if ui::button(
            Rect::new(x, 809.0, 344.0, 37.0),
            "USE CASE IN OBJECT EDITOR",
            ui_mouse,
            false,
        ) && self.error.is_none()
        {
            return Some(Action::Use(
                self.experiment.variants[self.selected].object.clone(),
            ));
        }
        None
    }
}
