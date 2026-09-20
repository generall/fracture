use super::*;
impl LevelEditor {
    fn resize_map(&mut self, axis: usize, value: f32) {
        let occupied = self
            .level
            .items
            .iter()
            .flat_map(|i| i.world_outline())
            .map(|p| p[axis].abs() * 2.0 + 0.402)
            .fold(12.0, f32::max);
        self.level.size[axis] = value.clamp(occupied.min(200.0), 200.0);
    }
    fn value(&self, field: Field) -> String {
        if field == Field::Name {
            return self.level.name.clone();
        }
        let item = self.selected.and_then(|i| self.level.items.get(i));
        let position = self.selection_center();
        let value = match field {
            Field::MapWidth => self.level.size[0],
            Field::MapHeight => self.level.size[1],
            Field::X => position.x,
            Field::Y => position.y,
            Field::Width => item.map_or(0.0, |i| i.dimensions().x),
            Field::Height => item.map_or(0.0, |i| i.dimensions().y),
            Field::Angle => item.map_or(0.0, |i| i.angle().to_degrees()),
            Field::GrainWidth | Field::GrainHeight => match item {
                Some(LevelItem::Object { asset, .. }) => asset
                    .layout
                    .as_ref()
                    .map_or(0.0, |l| l.size[usize::from(field == Field::GrainHeight)]),
                _ => 0.0,
            },
            Field::CaptureTime | Field::CaptureRadius | Field::RepairRate | Field::RadarRadius => {
                if let Some(LevelItem::Station { settings, .. }) = item {
                    match field {
                        Field::CaptureTime => settings.capture_time,
                        Field::CaptureRadius => settings.capture_radius,
                        Field::RepairRate => settings.repair_rate,
                        _ => settings.radar_radius,
                    }
                } else {
                    0.0
                }
            }
            Field::PickupRespawn => {
                if let Some(LevelItem::Pickup { respawn_time, .. }) = item {
                    *respawn_time
                } else {
                    0.0
                }
            }
            Field::Name => unreachable!(),
        };
        format!("{value:.2}")
    }
    pub(super) fn field(&mut self, field: Field, label: &str, rect: Rect, mouse: Vec2) {
        text(label, rect.x + 2.0, rect.y - 10.0, 14.0, MUTED);
        let active = self.editing.as_ref().is_some_and(|e| e.field == field);
        let value = self
            .editing
            .as_ref()
            .filter(|e| e.field == field)
            .map(|e| e.value.clone())
            .unwrap_or_else(|| self.value(field));
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::from_rgba(16, 23, 37, 255),
        );
        if active {
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, CYAN);
        }
        let limit = if field == Field::Name { 23 } else { 12 };
        let display: String = value
            .chars()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        text(
            &display,
            rect.x + 8.0,
            rect.y + 23.0,
            18.0,
            if active { CYAN } else { INK },
        );
        if rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) && !active {
            self.commit_field();
            self.editing = Some(Editing {
                field,
                value: self.value(field),
                fresh: true,
            });
            self.status = "Type a value. Enter applies; Escape cancels.".into();
            while get_char_pressed().is_some() {}
        }
    }
    pub(super) fn keyboard_fields(&mut self) {
        if self.editing.is_none() {
            while get_char_pressed().is_some() {}
            return;
        }
        if is_key_pressed(KeyCode::Escape) {
            self.editing = None;
            return;
        }
        let edit = self.editing.as_mut().unwrap();
        if (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
            && is_key_pressed(KeyCode::A)
        {
            edit.fresh = true;
        }
        if is_key_pressed(KeyCode::Backspace) {
            if edit.fresh {
                edit.value.clear();
                edit.fresh = false;
            } else {
                edit.value.pop();
            }
        }
        while let Some(c) = get_char_pressed() {
            if !c.is_control()
                && (edit.field == Field::Name
                    || c.is_ascii_digit()
                    || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
            {
                if edit.fresh {
                    edit.value.clear();
                    edit.fresh = false;
                }
                if edit.value.len() < 96 {
                    edit.value.push(c);
                }
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Tab) {
            self.commit_field();
        }
    }
    pub(super) fn commit_field(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        let before = self.level.clone();
        let result = (|| -> Result<(), String> {
            if edit.field == Field::Name {
                self.level.name = edit.value;
                return Ok(());
            }
            let n: f32 = edit
                .value
                .parse()
                .map_err(|_| "Enter a number.".to_string())?;
            if !n.is_finite() {
                return Err("Enter a finite number.".into());
            }
            match edit.field {
                Field::MapWidth => self.resize_map(0, n),
                Field::MapHeight => self.resize_map(1, n),
                field => {
                    let i = self.selected.ok_or("Select an item first.")?;
                    if matches!(field, Field::X | Field::Y) {
                        let pos = self.selection_center();
                        let desired = if field == Field::X {
                            sim::Vec2::new(n - pos.x, 0.0)
                        } else {
                            sim::Vec2::new(0.0, n - pos.y)
                        };
                        let delta = sim::level::clamp_translation(
                            &self.level.items,
                            &self.selection,
                            self.level.size.into(),
                            desired,
                        );
                        for &j in &self.selection {
                            self.level.items[j].translate(delta);
                        }
                        return Ok(());
                    }
                    let item = &mut self.level.items[i];
                    match field {
                        Field::X => item.translate(sim::Vec2::new(n - item.position().x, 0.0)),
                        Field::Y => item.translate(sim::Vec2::new(0.0, n - item.position().y)),
                        Field::Width => item.resize_inside(
                            sim::Vec2::new(n, item.dimensions().y),
                            self.level.size.into(),
                        )?,
                        Field::Height => item.resize_inside(
                            sim::Vec2::new(item.dimensions().x, n),
                            self.level.size.into(),
                        )?,
                        Field::Angle => match item {
                            LevelItem::Object { angle, .. }
                            | LevelItem::Solid { angle, .. }
                            | LevelItem::Spawn { angle, .. } => {
                                *angle = n.to_radians().rem_euclid(std::f32::consts::TAU)
                            }
                            _ => {}
                        },
                        Field::PickupRespawn => {
                            if let LevelItem::Pickup { respawn_time, .. } = item {
                                *respawn_time = n;
                            }
                        }
                        Field::CaptureTime
                        | Field::CaptureRadius
                        | Field::RepairRate
                        | Field::RadarRadius => {
                            if let LevelItem::Station { settings, .. } = item {
                                match field {
                                    Field::CaptureTime => settings.capture_time = n,
                                    Field::CaptureRadius => settings.capture_radius = n,
                                    Field::RepairRate => settings.repair_rate = n,
                                    _ => settings.radar_radius = n,
                                }
                            }
                        }
                        Field::GrainWidth | Field::GrainHeight => {
                            if let LevelItem::Object { asset, .. } = item
                                && let Some(l) = &asset.layout
                            {
                                let n = n.clamp(0.15, 200.0);
                                let mut size = l.size;
                                size[usize::from(field == Field::GrainHeight)] = n;
                                asset.regenerate_grains(l.mode, size, l.seed);
                                asset.fit_grain_budget();
                            }
                        }
                        _ => {}
                    }
                    item.fit_inside(self.level.size.into())?;
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.level = before;
            self.status = e;
        } else {
            self.finish_change(before);
            if matches!(edit.field, Field::MapWidth | Field::MapHeight) {
                self.fit_requested = true;
            }
        }
    }
}
