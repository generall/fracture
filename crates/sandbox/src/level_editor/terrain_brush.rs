//! Shared terrain and eraser strokes stay lightweight until the pointer is released.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Stroke {
    pub terrain: TerrainKind,
    pub cells: BTreeSet<(i32, i32)>, // Row, column: deterministic rectangle packing.
    grid: f32,
    last: Option<Vec2>,
}

impl Stroke {
    pub fn new(terrain: TerrainKind, grid: f32) -> Self {
        Self {
            terrain,
            grid,
            cells: BTreeSet::new(),
            last: None,
        }
    }

    pub fn pause(&mut self) {
        self.last = None;
    }

    pub fn extend(&mut self, point: Vec2, width: i32, size: Vec2) {
        let half = ((size * 0.5 - Vec2::splat(0.201)) / self.grid).floor();
        let width = (width as f32 / self.grid).round() as i32;
        let offset = Vec2::splat(if width % 2 == 0 { 1.0 } else { 0.5 });
        let end = (point / self.grid).floor().clamp(-half, half - Vec2::ONE) + offset;
        let start = self.last.unwrap_or(end);
        let steps = (start.distance(end) * 4.0).ceil().max(1.0) as usize;
        let radius = width as f32 * 0.5;
        for step in 0..=steps {
            let center = (start.lerp(end, step as f32 / steps as f32) - offset).round() + offset;
            for dy in 0..width {
                for dx in 0..width {
                    let delta = vec2(dx as f32, dy as f32) + Vec2::splat(0.5 - radius);
                    let p = center + delta;
                    if delta.length_squared() <= radius * radius
                        && p.cmpge(-half).all()
                        && p.cmplt(half).all()
                    {
                        self.cells.insert((p.y.floor() as i32, p.x.floor() as i32));
                    }
                }
            }
        }
        self.last = Some(end);
    }

    fn rectangles(&self) -> Vec<(Vec2, Vec2)> {
        let mut rows: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for &(y, x) in &self.cells {
            rows.entry(y).or_default().push(x);
        }
        let mut rectangles: Vec<(Vec2, Vec2)> = vec![];
        let mut previous = BTreeMap::new();
        for (y, xs) in rows {
            let mut current = BTreeMap::new();
            let mut i = 0;
            while i < xs.len() {
                let start = xs[i];
                let mut end = start + 1;
                i += 1;
                while i < xs.len() && xs[i] == end {
                    end += 1;
                    i += 1;
                }
                let key = (start, end);
                let index = previous
                    .get(&key)
                    .copied()
                    .filter(|&index: &usize| rectangles[index].1.y == y as f32);
                let index = if let Some(index) = index {
                    rectangles[index].1.y += 1.0;
                    index
                } else {
                    rectangles.push((
                        vec2(start as f32, y as f32),
                        vec2(end as f32, (y + 1) as f32),
                    ));
                    rectangles.len() - 1
                };
                current.insert(key, index);
            }
            previous = current;
        }
        rectangles
            .into_iter()
            .map(|(a, b)| (a * self.grid, b * self.grid))
            .collect()
    }

    pub fn apply(&self, level: &mut Level) {
        for (min, max) in self.rectangles() {
            level.paint_terrain(self.terrain, s(min), s(max));
        }
    }

    pub fn erase(&self, level: &mut Level) -> Result<(), String> {
        for (min, max) in self.rectangles() {
            level.erase_rectangle(s(min), s(max))?;
        }
        Ok(())
    }

    pub fn draw(&self, view: &Viewport, color: Color) {
        for &(y, x) in &self.cells {
            let a = view.screen(vec2(x as f32, y as f32) * self.grid);
            let b = view.screen(vec2(x as f32 + 1.0, y as f32 + 1.0) * self.grid);
            draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_metre_alignment_keeps_brush_width_and_erases_what_was_painted() {
        let mut stroke = Stroke::new(TerrainKind::Sand, 0.5);
        stroke.extend(vec2(0.6, -0.4), 1, Vec2::splat(30.0));
        assert_eq!(stroke.rectangles(), vec![(vec2(0.5, -0.5), vec2(1.5, 0.5))]);
        stroke.extend(vec2(5.6, -0.4), 1, Vec2::splat(30.0));
        assert_eq!(stroke.rectangles(), vec![(vec2(0.5, -0.5), vec2(6.5, 0.5))]);
        let mut level = Level {
            items: vec![],
            ..Level::default()
        };
        stroke.apply(&mut level);
        assert_eq!(level, Level::from_json(&level.to_json().unwrap()).unwrap());
        stroke.erase(&mut level).unwrap();
        assert!(level.items.is_empty());
    }

    #[test]
    fn eraser_follows_a_bent_stroke_instead_of_filling_its_bounds() {
        let mut level = Level {
            items: vec![
                LevelItem::Terrain {
                    terrain: TerrainKind::Water,
                    min: [-5.0, -5.0],
                    max: [5.0, 5.0],
                },
                LevelItem::Solid {
                    outline: sim::level::rectangle(sim::Vec2::splat(5.0)),
                    position: [0.0, 0.0],
                    angle: 0.0,
                    cutouts: vec![],
                },
            ],
            ..Level::default()
        };
        let mut stroke = Stroke::new(TerrainKind::Ground, 1.0);
        for point in [vec2(-3.0, -3.0), vec2(3.0, -3.0), vec2(3.0, 3.0)] {
            stroke.extend(point, 1, level.size.into());
        }
        stroke.erase(&mut level).unwrap();
        let mesh = level
            .resolved_meshes()
            .unwrap()
            .into_iter()
            .flatten()
            .next()
            .unwrap();
        assert!(mesh.contains_point(sim::Vec2::ZERO));
        for point in [sim::Vec2::new(0.5, -2.5), sim::Vec2::new(3.5, 0.5)] {
            assert!(!mesh.contains_point(point));
            assert!(!level.items.iter().any(|item| match item {
                LevelItem::Terrain { min, max, .. } =>
                    point.cmpge((*min).into()).all() && point.cmple((*max).into()).all(),
                _ => false,
            }));
        }
        assert_eq!(level, Level::from_json(&level.to_json().unwrap()).unwrap());
    }

    #[test]
    fn fast_stroke_is_continuous_clipped_and_packed() {
        let mut stroke = Stroke::new(TerrainKind::Road, 1.0);
        stroke.extend(vec2(-50.0, 0.1), 1, Vec2::splat(30.0));
        stroke.extend(vec2(50.0, 0.1), 1, Vec2::splat(30.0));
        assert_eq!(stroke.cells.len(), 28);
        assert_eq!(
            stroke.rectangles(),
            vec![(vec2(-14.0, 0.0), vec2(14.0, 1.0))]
        );
    }

    #[test]
    fn painting_cuts_water_preserves_objects_and_roundtrips() {
        let mut level = Level::default();
        let objects = level.items.clone();
        level.items.push(LevelItem::Terrain {
            terrain: TerrainKind::Water,
            min: [-5.0, -5.0],
            max: [5.0, 5.0],
        });
        let mut stroke = Stroke::new(TerrainKind::Sand, 1.0);
        stroke.extend(vec2(-6.0, 0.0), 3, level.size.into());
        stroke.extend(vec2(6.0, 0.0), 3, level.size.into());
        stroke.apply(&mut level);
        assert_eq!(&level.items[..objects.len()], &objects);
        assert_eq!(level.items.len(), objects.len());
        assert_eq!(
            level.terrain_mask.at(sim::Vec2::new(0.25, 0.25)),
            TerrainKind::Sand
        );
        assert_eq!(
            level.terrain_mask.at(sim::Vec2::new(0.25, 3.25)),
            TerrainKind::Water
        );
        assert_eq!(level, Level::from_json(&level.to_json().unwrap()).unwrap());
        let before = level.clone();
        stroke.apply(&mut level);
        assert_eq!(
            level, before,
            "repainting the same stroke must not add items"
        );
    }
}
