//! Field formatting, committing edits and keyboard entry.
use super::*;

impl Editor {
    pub(super) fn value(&self, field: Field) -> String {
        match field {
            Field::Name => self.asset.name.clone(),
            Field::YieldStress => format!(
                "{:.2}",
                self.asset
                    .components
                    .plasticity
                    .unwrap_or_default()
                    .yield_stress
                    / 1000.0
            ),
            Field::PlasticFlow => format!(
                "{:.2}",
                self.asset
                    .components
                    .plasticity
                    .unwrap_or_default()
                    .flow_rate
            ),
            Field::Hardening => format!(
                "{:.2}",
                self.asset
                    .components
                    .plasticity
                    .unwrap_or_default()
                    .hardening
            ),
            Field::PlasticLimit => format!(
                "{:.2}",
                self.asset
                    .components
                    .plasticity
                    .unwrap_or_default()
                    .max_strain
            ),
            Field::Color => self.asset.color.map_or_else(
                || "AUTO".into(),
                |[r, g, b]| format!("#{r:02X}{g:02X}{b:02X}"),
            ),
            Field::ProjectileResistance => {
                format!("{:.2}", self.asset.components.projectile_resistance)
            }
            Field::ContactResistance => format!("{:.2}", self.asset.components.contact_resistance),
            Field::CrushForce => format!(
                "{:.2}",
                self.asset.components.crush.unwrap_or_default().force / 1000.0
            ),
            Field::CrushResistance => format!(
                "{:.5}",
                self.asset.components.crush.unwrap_or_default().resistance
            ),
            Field::ExplosionEnergy => format!(
                "{:.1}",
                self.asset.components.explosive.unwrap_or_default().energy / 1000.0
            ),
            Field::ExplosionRadius => format!(
                "{:.2}",
                self.asset.components.explosive.unwrap_or_default().radius
            ),
            Field::Density => format!("{:.0}", self.asset.density),
            Field::Thickness => format!("{:.3}", self.asset.thickness),
            Field::Friction => format!("{:.2}", self.asset.friction),
            Field::Restitution => format!("{:.2}", self.asset.restitution),
            Field::SoftBounce => format!(
                "{:.2}",
                self.asset.soft_bounce.unwrap_or_default().restitution
            ),
            Field::BounceSpeed => {
                format!("{:.1}", self.asset.soft_bounce.unwrap_or_default().speed)
            }
            Field::Tension => format!("{:.1}", self.asset.tension / 1000.0),
            Field::Compression => format!("{:.1}", self.asset.compression / 1000.0),
            Field::Shear => format!("{:.1}", self.asset.shear / 1000.0),
            Field::Cohesion => format!("{:.1}", self.asset.cohesion),
            Field::X => format!("{:.3}", self.selected_point()[0]),
            Field::Y => format!("{:.3}", self.selected_point()[1]),
            Field::GrainWidth => format!(
                "{:.2}",
                self.asset.layout.as_ref().map_or(0.7, |l| l.size[0])
            ),
            Field::GrainHeight => format!(
                "{:.2}",
                self.asset.layout.as_ref().map_or(0.5, |l| l.size[1])
            ),
            Field::Seed => self.asset.layout.as_ref().map_or(7, |l| l.seed).to_string(),
            Field::Young => format!("{:.3}", self.asset.young_modulus() / 1e9),
            Field::Toughness => format!("{:.1}", self.asset.fracture_energy()),
        }
    }

    pub(super) fn commit_input(&mut self) -> bool {
        let Some(input) = self.input.take() else {
            return true;
        };
        if input.field == Field::Color {
            let s = input.text.trim().trim_start_matches('#');
            let color = if s.eq_ignore_ascii_case("auto") {
                None
            } else if s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit()) {
                let rgb = u32::from_str_radix(s, 16).unwrap();
                Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
            } else {
                self.status = "Color needs six hex digits, such as #69E0AE, or AUTO.".into();
                self.input = Some(input);
                return false;
            };
            self.checkpoint();
            self.asset.color = color;
            self.status = "Object color updated.".into();
            return true;
        }
        let value = if input.field == Field::Name {
            0.0
        } else {
            match input.text.trim().parse::<f32>() {
                Ok(v) if v.is_finite() => v,
                _ => {
                    self.status = "Enter a finite number. Escape cancels the edit.".into();
                    self.input = Some(input);
                    return false;
                }
            }
        };
        if input.field == Field::Seed && input.text.trim().parse::<u32>().is_err() {
            self.status = "Seed must be a whole number from 0 to 4294967295.".into();
            self.input = Some(input);
            return false;
        }
        self.checkpoint();
        match input.field {
            Field::Name => {
                // Carry the constraint forward when a legacy named barrel is renamed.
                self.asset.circular |= self.asset.is_circular();
                self.asset.name = input.text.trim().into();
            }
            Field::YieldStress => {
                self.asset
                    .components
                    .plasticity
                    .get_or_insert_default()
                    .yield_stress = value * 1000.0
            }
            Field::PlasticFlow => {
                self.asset
                    .components
                    .plasticity
                    .get_or_insert_default()
                    .flow_rate = value
            }
            Field::Hardening => {
                self.asset
                    .components
                    .plasticity
                    .get_or_insert_default()
                    .hardening = value
            }
            Field::PlasticLimit => {
                self.asset
                    .components
                    .plasticity
                    .get_or_insert_default()
                    .max_strain = value
            }
            Field::Color => unreachable!(),
            Field::ProjectileResistance => self.asset.components.projectile_resistance = value,
            Field::ContactResistance => self.asset.components.contact_resistance = value,
            Field::CrushForce => {
                self.asset.components.crush.get_or_insert_default().force = value * 1000.0
            }
            Field::CrushResistance => {
                self.asset
                    .components
                    .crush
                    .get_or_insert_default()
                    .resistance = value
            }
            Field::ExplosionEnergy => {
                self.asset
                    .components
                    .explosive
                    .get_or_insert_default()
                    .energy = value * 1000.0
            }
            Field::ExplosionRadius => {
                self.asset
                    .components
                    .explosive
                    .get_or_insert_default()
                    .radius = value
            }
            Field::Density => self.asset.density = value,
            Field::Thickness => self.asset.thickness = value,
            Field::Friction => self.asset.friction = value,
            Field::Restitution => self.asset.restitution = value,
            Field::SoftBounce => self.asset.soft_bounce.get_or_insert_default().restitution = value,
            Field::BounceSpeed => self.asset.soft_bounce.get_or_insert_default().speed = value,
            Field::Tension => self.asset.tension = value * 1000.0,
            Field::Compression => self.asset.compression = value * 1000.0,
            Field::Shear => self.asset.shear = value * 1000.0,
            Field::Cohesion => self.asset.cohesion = value,
            Field::X | Field::Y => {
                let mut p = self.selected_point();
                p[usize::from(input.field == Field::Y)] = value;
                self.set_point(p);
            }
            Field::Young => self.asset.young = Some(value * 1e9),
            Field::Toughness => self.asset.toughness = Some(value),
            Field::GrainWidth | Field::GrainHeight | Field::Seed => {
                let layout = self.asset.layout.as_ref();
                let mut size = layout.map_or([0.7, 0.5], |l| l.size);
                let mut seed = layout.map_or(7, |l| l.seed);
                if input.field == Field::Seed {
                    seed = input.text.trim().parse::<u32>().unwrap();
                } else {
                    size[usize::from(input.field == Field::GrainHeight)] = value;
                }
                self.regenerate(layout.map_or(GrainMode::Stones, |l| l.mode), size, seed);
            }
        }
        self.status = "Object updated.".into();
        true
    }

    pub(super) fn field(&mut self, field: Field, label: &str, y: f32, mouse: Vec2) {
        let rect = if field == Field::Name {
            Rect::new(108.0, y, 246.0, 32.0)
        } else {
            Rect::new(214.0, y, 140.0, 32.0)
        };
        text(label, 28.0, y + 22.0, 16.0, MUTED);
        self.field_in(field, rect, mouse);
    }

    pub(super) fn field_in(&mut self, field: Field, rect: Rect, mouse: Vec2) {
        let active = self.input.as_ref().is_some_and(|i| i.field == field);
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
            self.input.as_ref().unwrap().text.clone()
        } else {
            self.value(field)
        };
        let count = ((rect.w - 16.0) / 10.8) as usize;
        let visible: String = if active {
            value
                .chars()
                .rev()
                .take(count)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        } else if value.chars().count() > count {
            format!("{}..", value.chars().take(count - 2).collect::<String>())
        } else {
            value
        };
        if active && self.input.as_ref().unwrap().all {
            draw_rectangle(
                rect.x + 5.0,
                rect.y + 3.0,
                rect.w - 10.0,
                26.0,
                Color::new(0.10, 0.26, 0.36, 1.0),
            );
        }
        crate::ui::field_text(&visible, rect, 22.0, 18.0, active);
        if rect.contains(mouse)
            && is_mouse_button_pressed(MouseButton::Left)
            && !active
            && self.commit_input()
        {
            self.input = Some(Input {
                field,
                text: self.value(field),
                all: true,
                rect,
            });
        }
    }

    pub(super) fn keyboard_input(&mut self) {
        let ctrl = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || is_key_down(KeyCode::LeftSuper)
            || is_key_down(KeyCode::RightSuper);
        if let Some(input) = &mut self.input {
            if ctrl && is_key_pressed(KeyCode::A) {
                input.all = true;
            }
            if is_key_pressed(KeyCode::Backspace) {
                if input.all {
                    input.text.clear();
                } else {
                    input.text.pop();
                }
                input.all = false;
            }
            while let Some(c) = get_char_pressed() {
                if !ctrl
                    && !c.is_control()
                    && (matches!(input.field, Field::Name | Field::Color)
                        || c.is_ascii_digit()
                        || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
                {
                    if input.all {
                        input.text.clear();
                        input.all = false;
                    }
                    if input.text.chars().count() < 32 {
                        input.text.push(c);
                    }
                }
            }
            if is_key_pressed(KeyCode::Escape) {
                self.input = None;
            } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Tab) {
                self.commit_input();
            }
        } else {
            while get_char_pressed().is_some() {}
        }
    }
}
