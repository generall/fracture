//! Material/impact field formatting, validation and keyboard entry.
use super::*;

impl Comparison {
    pub(super) fn value(&self, field: Field) -> String {
        let v = &self.experiment.variants[self.selected];
        let a = &v.object;
        let i = &v.impact;
        let l = a.layout.as_ref();
        match field {
            Field::Name => self.experiment.name.clone(),
            Field::YieldStress => format!(
                "{}",
                a.components.plasticity.unwrap_or_default().yield_stress / 1000.0
            ),
            Field::PlasticFlow => {
                format!("{}", a.components.plasticity.unwrap_or_default().flow_rate)
            }
            Field::Hardening => {
                format!("{}", a.components.plasticity.unwrap_or_default().hardening)
            }
            Field::StrainLimit => {
                format!("{}", a.components.plasticity.unwrap_or_default().max_strain)
            }
            Field::Variant => v.name.clone(),
            Field::Density => format!("{}", a.density),
            Field::Young => format!("{}", a.young_modulus() / 1e9),
            Field::Toughness => format!("{}", a.fracture_energy()),
            Field::Tension => format!("{}", a.tension / 1000.0),
            Field::Compression => format!("{}", a.compression / 1000.0),
            Field::Shear => format!("{}", a.shear / 1000.0),
            Field::Cohesion => format!("{}", a.cohesion),
            Field::Friction => format!("{}", a.friction),
            Field::Restitution => format!("{}", a.restitution),
            Field::SoftBounce => format!("{}", a.soft_bounce.unwrap_or_default().restitution),
            Field::BounceSpeed => format!("{}", a.soft_bounce.unwrap_or_default().speed),
            Field::Depth => format!("{}", a.thickness),
            Field::ShellMass => format!("{}", i.shell_mass),
            Field::ShellSpeed => format!("{}", i.shell_speed),
            Field::TankMass => format!("{}", i.tank_mass),
            Field::TankSpeed => format!("{}", i.tank_speed),
            Field::TankAngle => format!("{}", i.angle_degrees),
            Field::AimX => format!("{}", i.aim[0]),
            Field::AimY => format!("{}", i.aim[1]),
            Field::Duration => format!("{}", self.experiment.duration_ticks as f32 / 120.0),
            Field::GrainWidth => format!("{}", l.map_or(0.7, |l| l.size[0])),
            Field::GrainHeight => format!("{}", l.map_or(0.5, |l| l.size[1])),
            Field::Seed => format!("{}", l.map_or(7, |l| l.seed)),
            Field::Detail => format!("{}", a.refinement),
        }
    }
    pub(super) fn commit(&mut self) -> bool {
        let Some(input) = self.input.take() else {
            return true;
        };
        let mut e = self.experiment.clone();
        let v = &mut e.variants[self.selected];
        let a = &mut v.object;
        let i = &mut v.impact;
        if input.field == Field::Name {
            e.name = input.value.clone();
        } else if input.field == Field::Variant {
            v.name = input.value.clone();
        } else {
            let Ok(n) = input.value.parse::<f32>() else {
                self.message = "Enter a number, then press Enter.".into();
                self.input = Some(input);
                return false;
            };
            if !n.is_finite() {
                self.message = "Enter a finite number.".into();
                self.input = Some(input);
                return false;
            }
            match input.field {
                Field::YieldStress => {
                    a.components.plasticity.get_or_insert_default().yield_stress = n * 1000.0
                }
                Field::PlasticFlow => a.components.plasticity.get_or_insert_default().flow_rate = n,
                Field::Hardening => a.components.plasticity.get_or_insert_default().hardening = n,
                Field::StrainLimit => {
                    a.components.plasticity.get_or_insert_default().max_strain = n
                }
                Field::Density => a.density = n,
                Field::Young => a.young = Some(n * 1e9),
                Field::Toughness => a.toughness = Some(n),
                Field::Tension => a.tension = n * 1000.0,
                Field::Compression => a.compression = n * 1000.0,
                Field::Shear => a.shear = n * 1000.0,
                Field::Cohesion => a.cohesion = n,
                Field::Friction => a.friction = n,
                Field::Restitution => a.restitution = n,
                Field::SoftBounce => a.soft_bounce.get_or_insert_default().restitution = n,
                Field::BounceSpeed => a.soft_bounce.get_or_insert_default().speed = n,
                Field::Depth => a.thickness = n,
                Field::ShellMass => i.shell_mass = n,
                Field::ShellSpeed => i.shell_speed = n,
                Field::TankMass => i.tank_mass = n,
                Field::TankSpeed => i.tank_speed = n,
                Field::TankAngle => i.angle_degrees = n,
                Field::AimX => i.aim[0] = n,
                Field::AimY => i.aim[1] = n,
                Field::Duration => e.duration_ticks = (n * 120.0).round() as u32,
                Field::GrainWidth | Field::GrainHeight | Field::Seed => {
                    let l = a.layout.clone();
                    let mut size = l.as_ref().map_or([0.7, 0.5], |l| l.size);
                    let mut seed = l.as_ref().map_or(7, |l| l.seed);
                    if input.field == Field::Seed {
                        let Ok(s) = input.value.parse::<u32>() else {
                            self.message =
                                "Seed must be a whole number from 0 to 4294967295.".into();
                            self.input = Some(input);
                            return false;
                        };
                        seed = s;
                    } else {
                        size[usize::from(input.field == Field::GrainHeight)] = n;
                    }
                    // Validate before generation: avoid loops from invalid sizes.
                    if size.iter().any(|n| !(0.15..=4.0).contains(n)) {
                        self.message = "Grain sizes must be between 0.15 and 4 metres.".into();
                        self.input = Some(input);
                        return false;
                    }
                    a.regenerate_grains(l.map_or(GrainMode::Stones, |l| l.mode), size, seed);
                }
                Field::Detail => {
                    if !(0.0..=4.0).contains(&n) || n.fract() != 0.0 {
                        self.message = "Detail must be a whole number from 0 to 4.".into();
                        self.input = Some(input);
                        return false;
                    }
                    a.refinement = n as u8;
                }
                Field::Name | Field::Variant => unreachable!(),
            }
        }
        // Accept invalid meshes so a second edit can repair them. The range
        // shows the validation error and never runs or saves invalid cases.
        let metadata = matches!(input.field, Field::Name | Field::Variant);
        if input.shared {
            e.shared_impact = true;
        }
        self.experiment = e;
        if metadata {
            self.error = self.experiment.validate().err();
        } else {
            self.changed();
        }
        true
    }
    pub(super) fn field(&mut self, field: Field, label: &str, rect: Rect, mouse: Vec2) {
        self.field_scoped(field, label, rect, mouse, false);
    }
    pub(super) fn field_scoped(
        &mut self,
        field: Field,
        label: &str,
        rect: Rect,
        mouse: Vec2,
        shared: bool,
    ) {
        ui::text(label, rect.x, rect.y - 7.0, 15.0, MUTED);
        let active = self
            .input
            .as_ref()
            .is_some_and(|i| i.field == field && i.shared == shared);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(0.055, 0.085, 0.15, 1.0),
        );
        draw_line(
            rect.x,
            rect.bottom(),
            rect.right(),
            rect.bottom(),
            if active { 2.0 } else { 1.0 },
            if active { CYAN } else { RULE },
        );
        let value = if active {
            self.input.as_ref().unwrap().value.clone()
        } else {
            self.value(field)
        };
        let count = ((rect.w - 16.0) / 10.8) as usize;
        let visible = if value.chars().count() > count {
            if active {
                value.chars().skip(value.chars().count() - count).collect()
            } else {
                format!(
                    "{}..",
                    value
                        .chars()
                        .take(count.saturating_sub(2))
                        .collect::<String>()
                )
            }
        } else {
            value
        };
        if active && self.input.as_ref().unwrap().all {
            draw_rectangle(
                rect.x + 5.0,
                rect.y + 3.0,
                rect.w - 10.0,
                rect.h - 6.0,
                Color::new(0.1, 0.26, 0.36, 1.0),
            );
        }
        ui::text(
            &visible,
            rect.x + 8.0,
            rect.y + 23.0,
            18.0,
            if active { CYAN } else { INK },
        );
        if rect.contains(mouse)
            && is_mouse_button_pressed(MouseButton::Left)
            && !active
            && self.commit()
        {
            self.input = Some(Input {
                field,
                value: self.value(field),
                all: true,
                rect,
                shared,
            });
        }
    }
    pub(super) fn keyboard(&mut self, mouse: Vec2) {
        if self.input.is_some() {
            if is_key_pressed(KeyCode::Escape) {
                self.input = None;
                return;
            }
            if is_key_pressed(KeyCode::Enter) {
                self.commit();
                return;
            }
            if let Some(i) = &mut self.input {
                if (is_key_down(KeyCode::LeftControl)
                    || is_key_down(KeyCode::RightControl)
                    || is_key_down(KeyCode::LeftSuper))
                    && is_key_pressed(KeyCode::A)
                {
                    i.all = true;
                }
                if is_key_pressed(KeyCode::Backspace) {
                    if i.all {
                        i.value.clear();
                    } else {
                        i.value.pop();
                    }
                    i.all = false;
                }
                while let Some(c) = get_char_pressed() {
                    if !c.is_control() && i.value.chars().count() < 80 {
                        if i.all {
                            i.value.clear();
                        }
                        i.all = false;
                        i.value.push(c);
                    }
                }
                if is_mouse_button_pressed(MouseButton::Left) && !i.rect.contains(mouse) {
                    self.commit();
                }
            }
        } else {
            while get_char_pressed().is_some() {}
            if is_key_pressed(KeyCode::Space) {
                self.playing = !self.playing && self.runner.is_some();
            }
            if is_key_pressed(KeyCode::N) {
                self.playing = false;
                self.cursor = (self.cursor + FRAME_TICKS as f32).min(self.available() as f32);
            }
            if is_key_pressed(KeyCode::T) {
                self.slow = !self.slow;
            }
            if is_key_pressed(KeyCode::Tab) {
                self.stress = !self.stress;
            }
        }
    }
}
