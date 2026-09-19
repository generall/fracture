//! Comparison toolbar, linked playback and card layout.
use super::*;

impl Comparison {
    pub fn draw(&mut self, size: Vec2, mouse: Vec2, dt: f32, pixel_scale: f32) -> Option<Action> {
        if let Some(result) = storage::take_import() {
            match result.and_then(|s| Experiment::from_json(&s)) {
                Ok(e) => self.load(e),
                Err(e) => self.message = e,
            }
        }
        let was_editing = self.input.is_some();
        if !self.library_open {
            self.keyboard(mouse);
        }
        let ready = self.input.is_none() && !self.library_open;
        let ui_mouse = if ready { mouse } else { vec2(-100.0, -100.0) };
        if self.calculating {
            let start = get_time();
            if let Some(r) = &mut self.runner {
                while !r.finished() && get_time() - start < 0.006 {
                    r.advance();
                }
                if r.finished() {
                    self.calculating = false;
                    self.message = if r.memory_limited {
                        "Replay memory limit reached. Shorten duration or use fewer cells."
                    } else {
                        "Recording complete. Scrub, pick a favorite, then SAVE or EXPORT."
                    }
                    .into();
                }
            }
        }
        if self.playing {
            self.cursor = (self.cursor + dt * 120.0 * if self.slow { 0.2 } else { 1.0 })
                .min(self.available() as f32);
            if !self.calculating && self.cursor >= self.available() as f32 {
                if self.looping {
                    self.cursor = 0.0;
                } else {
                    self.playing = false;
                }
            }
        }
        let compact = self.experiment.variants.len() > 4 && !self.focus;
        let side = if compact { size.x } else { size.x - 376.0 };
        ui::text("FRACTURE / COMPARISON RANGE", 28.0, 43.0, 26.0, INK);
        ui::text(
            &format!(
                "ONE RECIPE / {} CASES / SHARED TIME",
                self.experiment.variants.len()
            ),
            28.0,
            68.0,
            15.0,
            MUTED,
        );
        let controls = [
            (580.0, 80.0, if self.calculating { "STOP" } else { "RUN" }),
            (662.0, 92.0, if self.playing { "PAUSE" } else { "PLAY" }),
            (756.0, 94.0, "REWIND"),
            (852.0, 75.0, "SAVE"),
            (929.0, 88.0, "EXPORT"),
            (1019.0, 88.0, "IMPORT"),
            (1109.0, 104.0, "LIBRARY"),
            (size.x - 194.0, 170.0, "O  EDITOR"),
        ];
        for (j, (x, w, label)) in controls.into_iter().enumerate() {
            if ui::button(
                Rect::new(x, 24.0, w, 38.0),
                label,
                ui_mouse,
                j == 0 && self.calculating,
            ) {
                match j {
                    0 => {
                        if self.calculating {
                            self.calculating = false;
                            self.playing = false;
                            self.message = "Recording stopped. RUN restarts this recipe.".into();
                        } else {
                            self.run()
                        }
                    }
                    1 => {
                        if self.runner.is_some() {
                            self.playing = !self.playing;
                        } else {
                            self.message =
                                "RUN to reconstruct the timeline from this saved frame.".into();
                        }
                    }
                    2 => {
                        if self.runner.is_some() {
                            self.cursor = 0.0;
                            self.playing = false;
                        }
                    }
                    3 => self.save(),
                    4 => self.export(),
                    5 => {
                        if let Err(e) = storage::pick_import() {
                            self.message = e;
                        }
                    }
                    6 => self.library_open = true,
                    7 => return Some(Action::Close),
                    _ => {}
                }
            }
        }
        if ready && !was_editing && (is_key_pressed(KeyCode::O) || is_key_pressed(KeyCode::Escape))
        {
            return Some(Action::Close);
        }
        draw_line(24.0, 81.0, size.x - 24.0, 81.0, 1.0, RULE);
        self.field(
            Field::Name,
            "EXPERIMENT / SAVE UPDATES THIS NAME",
            Rect::new(28.0, 109.0, 460.0, 33.0),
            if self.library_open {
                vec2(-100.0, -100.0)
            } else {
                mouse
            },
        );
        if ui::button(
            Rect::new(506.0, 107.0, 178.0, 36.0),
            if self.focus {
                "ALL VIEWS"
            } else {
                "FOCUS CASE"
            },
            ui_mouse,
            self.focus,
        ) {
            self.focus = !self.focus;
            self.zoom_anchor = None;
        }
        if ui::button(
            Rect::new(690.0, 107.0, 180.0, 36.0),
            "TAB SOLVER MESH",
            ui_mouse,
            self.stress,
        ) {
            self.stress = !self.stress;
        }
        if ui::button(
            Rect::new(
                874.0,
                107.0,
                if compact { 164.0 } else { side - 898.0 },
                36.0,
            ),
            "FIT",
            ui_mouse,
            false,
        ) {
            self.fit_view();
        }
        if compact
            && ui::button(
                Rect::new(1064.0, 107.0, size.x - 1088.0, 36.0),
                "MARK SELECTED FAVORITE",
                ui_mouse,
                self.experiment.favorite == Some(self.selected),
            )
        {
            self.experiment.favorite = Some(self.selected);
            self.message = format!(
                "Case {:02} marked. SAVE keeps your choice; FOCUS CASE opens its settings.",
                self.selected + 1
            );
        }
        if compact {
            self.shared_test_bar(mouse, ui_mouse);
        }
        let top = if compact { 233.0 } else { 157.0 };
        let area = Rect::new(24.0, top, side - 48.0, size.y - top - 148.0);
        let cards = self.cards(area);
        for (_, card) in &cards {
            let rect = card_view(*card, compact);
            if ready && rect.contains(mouse) {
                if is_mouse_button_down(MouseButton::Right) {
                    self.center -= (mouse - self.last_mouse) / view_scale(rect, self.zoom);
                    self.zoom_anchor = None;
                }
                let wheel = mouse_wheel().1;
                if wheel != 0.0 {
                    let world = self.center + (mouse - rect.center()) / view_scale(rect, self.zoom);
                    self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(0.5, 6.0);
                    self.zoom_anchor = Some((world, mouse, rect));
                }
            }
        }
        self.zoom = (self.zoom.ln()
            + (self.target_zoom.ln() - self.zoom.ln()) * (1.0 - (-14.0 * dt).exp()))
        .exp();
        if let Some((world, anchor, rect)) = self.zoom_anchor {
            self.center = world - (anchor - rect.center()) / view_scale(rect, self.zoom);
        }
        self.last_mouse = mouse;
        for (i, card) in cards {
            if ready && card.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
                self.selected = i;
            }
            draw_rectangle_lines(
                card.x,
                card.y,
                card.w,
                card.h,
                if i == self.selected { 2.0 } else { 1.0 },
                if i == self.selected { CYAN } else { RULE },
            );
            let v = &self.experiment.variants[i];
            let favorite = self.experiment.favorite == Some(i);
            ui::text(
                &format!(
                    "{} {}",
                    if favorite { "*" } else { " " },
                    v.name
                        .chars()
                        .take(((card.w - 85.0) / 10.8) as usize)
                        .collect::<String>()
                ),
                card.x + 10.0,
                card.y + 26.0,
                18.0,
                if favorite { PINK } else { INK },
            );
            ui::right_text(
                if v.impact.kind == ImpactKind::Shell {
                    v.impact.ammunition.label()
                } else if v.impact.backstop {
                    "CRUSH"
                } else {
                    "TANK"
                },
                card.right() - 12.0,
                card.y + 26.0,
                14.0,
                MUTED,
            );
            let rect = card_view(card, compact);
            if let Some(state) = self.state(i) {
                draw_state(
                    &state.geometry,
                    &state.frame,
                    rect,
                    self.center,
                    self.zoom,
                    self.stress,
                    pixel_scale,
                );
                let m = &state.frame.metrics;
                if compact {
                    let varied_impact = self
                        .experiment
                        .variants
                        .iter()
                        .any(|other| other.impact != self.experiment.variants[0].impact);
                    let varied_limit = self.experiment.variants.iter().any(|other| {
                        other.object.components.plasticity.map(|p| p.max_strain)
                            != self.experiment.variants[0]
                                .object
                                .components
                                .plasticity
                                .map(|p| p.max_strain)
                    });
                    let varied_contact = self.experiment.variants.iter().any(|other| {
                        other.object.friction != self.experiment.variants[0].object.friction
                            || other.object.restitution
                                != self.experiment.variants[0].object.restitution
                    });
                    let parameter = if let Some(soft) = v.object.soft_bounce {
                        format!(
                            "BOUNCE {:.2} > {:.2} / {:.0}m/s",
                            soft.restitution, v.object.restitution, soft.speed
                        )
                    } else if varied_contact {
                        format!(
                            "FRICTION {:.2} / BOUNCE {:.2}",
                            v.object.friction, v.object.restitution
                        )
                    } else if let Some(p) = v.object.components.plasticity.filter(|_| varied_limit)
                    {
                        format!("STRAIN LIMIT {:.2}", p.max_strain)
                    } else if varied_impact && v.impact.kind == ImpactKind::Tank {
                        format!(
                            "TANK {:.1} m/s / OFFSET {:+.2} m",
                            v.impact.tank_speed, v.impact.aim[1]
                        )
                    } else if let Some(p) = v.object.components.plasticity {
                        format!("YIELD {:.1} kPa", p.yield_stress / 1000.0)
                    } else if let Some(c) = v.object.components.crush {
                        format!("CRUSH SCALE {:.4}x", c.resistance)
                    } else {
                        format!("Gc {:.0} J/m2", v.object.fracture_energy())
                    };
                    ui::text(&parameter, card.x + 12.0, card.bottom() - 49.0, 14.0, MUTED);
                    ui::text(
                        &if v.object.components.plasticity.is_some() {
                            format!("{:.0}% PERMANENT STRAIN", m.plastic_strain * 100.0)
                        } else {
                            format!("{} CHUNKS / {:.2}m2 CORE", m.chunks, m.largest_area)
                        },
                        card.x + 12.0,
                        card.bottom() - 28.0,
                        14.0,
                        INK,
                    );
                    ui::text(
                        &if m.unconverged > 0 {
                            "SOLVER LIMIT".into()
                        } else if v.object.components.plasticity.is_some() {
                            format!("{:.1}J PLASTIC WORK", m.plastic_work)
                        } else {
                            format!("{} CRACKS / {:.2}J WORK", m.broken, m.fracture_work)
                        },
                        card.x + 12.0,
                        card.bottom() - 9.0,
                        14.0,
                        if m.unconverged > 0 { PINK } else { VIOLET },
                    );
                    continue;
                }
                let y = card.bottom() - 54.0;
                ui::text(
                    &if v.object.components.plasticity.is_some() {
                        format!("DEFORMABLE FRAME / {:.0}% STRAIN", m.plastic_strain * 100.0)
                    } else {
                        format!("{} CHUNKS / {} BROKEN SEAMS", m.chunks, m.broken)
                    },
                    card.x + 12.0,
                    y,
                    15.0,
                    VIOLET,
                );
                ui::text(
                    &format!(
                        "LARGEST {:.2}m2 / REMOVED {:.2}m2",
                        m.largest_area,
                        m.cleared_area + m.ablated_area
                    ),
                    card.x + 12.0,
                    y + 21.0,
                    15.0,
                    INK,
                );
                ui::text(
                    &if m.unconverged > 0 {
                        format!("SOLVER LIMIT ({}) / REDUCE DETAIL", m.unconverged)
                    } else if v.object.components.plasticity.is_some() {
                        format!(
                            "PLASTIC WORK {:.1}J / PEAK ELASTIC {:.2}J",
                            m.plastic_work, m.elastic_energy
                        )
                    } else {
                        format!(
                            "Gc WORK {:.1}J / PEAK ELASTIC {:.2}J",
                            m.fracture_work, m.elastic_energy
                        )
                    },
                    card.x + 12.0,
                    y + 42.0,
                    14.0,
                    if m.unconverged > 0 { PINK } else { MUTED },
                );
            } else {
                ui::text(
                    "ADJUST SETTINGS TO RUN",
                    rect.x + 12.0,
                    rect.center().y,
                    18.0,
                    PINK,
                );
            }
        }
        let y = size.y - 124.0;
        let timeline = Rect::new(28.0, y + 31.0, side - 56.0, 20.0);
        ui::text(
            &format!(
                "{:05.2}s / {:.1}s",
                (self.cursor as u32 / FRAME_TICKS * FRAME_TICKS) as f32 / 120.0,
                self.experiment.duration_ticks as f32 / 120.0
            ),
            28.0,
            y + 15.0,
            20.0,
            CYAN,
        );
        ui::right_text(
            &format!(
                "{} {:.0}% / SPACE PLAY / N STEP",
                if self.calculating {
                    "COMPUTING"
                } else {
                    "RECORDED"
                },
                100.0 * self.available() as f32 / self.experiment.duration_ticks as f32
            ),
            side - 28.0,
            y + 15.0,
            15.0,
            MUTED,
        );
        draw_rectangle(timeline.x, timeline.y + 7.0, timeline.w, 5.0, RULE);
        draw_rectangle(
            timeline.x,
            timeline.y + 7.0,
            timeline.w * self.available() as f32 / self.experiment.duration_ticks as f32,
            5.0,
            VIOLET,
        );
        let x = timeline.x + timeline.w * self.cursor / self.experiment.duration_ticks as f32;
        draw_line(x, timeline.y, x, timeline.bottom(), 3.0, CYAN);
        if ready
            && self.runner.is_some()
            && timeline.contains(mouse)
            && is_mouse_button_down(MouseButton::Left)
        {
            self.playing = false;
            self.cursor = (((mouse.x - timeline.x) / timeline.w
                * self.experiment.duration_ticks as f32) as u32
                / FRAME_TICKS
                * FRAME_TICKS)
                .min(self.available()) as f32;
        }
        if ui::button(
            Rect::new(side - 155.0, y + 55.0, 130.0, 33.0),
            if self.slow { "T  0.2x" } else { "T  1.0x" },
            ui_mouse,
            self.slow,
        ) {
            self.slow = !self.slow;
        }
        if ui::button(
            Rect::new(side - 291.0, y + 55.0, 130.0, 33.0),
            "LOOP",
            ui_mouse,
            self.looping,
        ) {
            self.looping = !self.looping;
        }
        ui::text(
            "WHEEL ZOOM / RIGHT-DRAG PAN / ALL VIEWS LINKED",
            28.0,
            y + 77.0,
            15.0,
            MUTED,
        );
        let action = if compact {
            None
        } else {
            self.sidebar(side, mouse, ui_mouse)
        };
        draw_line(24.0, size.y - 25.0, size.x - 24.0, size.y - 25.0, 1.0, RULE);
        let notice = self.error.as_deref().unwrap_or(&self.message);
        let count = ((size.x - 56.0) / 9.0) as usize;
        ui::text(
            &notice.chars().take(count).collect::<String>(),
            28.0,
            size.y - 7.0,
            15.0,
            if self.error.is_some() { PINK } else { INK },
        );
        if self.library_open {
            self.library(size, mouse);
        }
        publish(self);
        action
    }
}
