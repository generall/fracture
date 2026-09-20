//! Fixed 0.5 m cells, independent of editable objects. JSON stores trimmed rows.
use super::*;
use crate::TerrainPatch;
use std::collections::BTreeMap;

pub const TERRAIN_CELL_SIZE: f32 = 0.5;
const SIDE: usize = 400;
const OFFSET: i32 = 200;
const KINDS: [TerrainKind; 7] = [
    TerrainKind::Ground,
    TerrainKind::Road,
    TerrainKind::Sand,
    TerrainKind::Water,
    TerrainKind::TallGrass,
    TerrainKind::Elevation,
    TerrainKind::Cliff,
];
const CODES: &[u8; 7] = b"grswtec";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Rows", into = "Rows")]
pub struct TerrainMask {
    cells: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rows {
    rows: Vec<Row>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    x: i32,
    y: i32,
    tiles: String,
}

impl From<TerrainMask> for Rows {
    fn from(mask: TerrainMask) -> Self {
        let mut rows = vec![];
        for (y, cells) in mask.cells.chunks(SIDE).enumerate() {
            if let Some(start) = cells.iter().position(|&c| c != 0) {
                let end = cells.iter().rposition(|&c| c != 0).unwrap() + 1;
                rows.push(Row {
                    x: start as i32 - OFFSET,
                    y: y as i32 - OFFSET,
                    tiles: cells[start..end]
                        .iter()
                        .map(|&c| CODES[c as usize] as char)
                        .collect(),
                });
            }
        }
        Self { rows }
    }
}
impl TryFrom<Rows> for TerrainMask {
    type Error = String;
    fn try_from(value: Rows) -> Result<Self, String> {
        if value.rows.len() > SIDE {
            return Err("Terrain mask has too many rows.".into());
        }
        let mut result = Self::default();
        let mut seen = [false; SIDE];
        for row in value.rows {
            if !(-OFFSET..OFFSET).contains(&row.y)
                || !(-OFFSET..OFFSET).contains(&row.x)
                || row.tiles.is_empty()
                || row.tiles.len() > SIDE
                || row.x + row.tiles.len() as i32 > OFFSET
            {
                return Err("Terrain mask row exceeds the 200 m map limit.".into());
            }
            let y = (row.y + OFFSET) as usize;
            if seen[y] {
                return Err("Terrain mask contains a duplicate row.".into());
            }
            seen[y] = true;
            result.cells.resize(SIDE * SIDE, 0);
            for (i, code) in row.tiles.bytes().enumerate() {
                let kind = CODES
                    .iter()
                    .position(|&c| c == code)
                    .ok_or("Unknown terrain mask tile.")?;
                result.cells[y * SIDE + (row.x + OFFSET) as usize + i] = kind as u8;
            }
        }
        result.compact_empty();
        Ok(result)
    }
}

impl TerrainMask {
    pub(super) fn validate(&self, size: Vec2) -> Result<(), String> {
        let half = ((size * 0.5 - Vec2::splat(0.201)) / TERRAIN_CELL_SIZE).floor();
        let left = (OFFSET - half.x as i32) as usize;
        let right = (OFFSET + half.x as i32) as usize;
        for (y, row) in self.cells.chunks(SIDE).enumerate() {
            let y = y as i32 - OFFSET;
            let outside = if y < -(half.y as i32) || y >= half.y as i32 {
                row.iter().any(|&c| c != 0)
            } else {
                row[..left].iter().chain(&row[right..]).any(|&c| c != 0)
            };
            if outside {
                return Err("Terrain mask extends beyond the map boundary.".into());
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn at(&self, point: Vec2) -> TerrainKind {
        if !point.is_finite() {
            return TerrainKind::Ground;
        }
        let cell = (point / TERRAIN_CELL_SIZE).floor();
        if cell.x < -200.0 || cell.x >= 200.0 || cell.y < -200.0 || cell.y >= 200.0 {
            return TerrainKind::Ground;
        }
        let index = (cell.y as i32 + OFFSET) as usize * SIDE + (cell.x as i32 + OFFSET) as usize;
        KINDS[self.cells.get(index).copied().unwrap_or(0) as usize]
    }

    /// Cell centres choose coverage. Authored brush edges already align exactly.
    pub fn paint(&mut self, kind: TerrainKind, min: Vec2, max: Vec2, size: Vec2) -> usize {
        let Some(code) = KINDS.iter().position(|&k| k == kind) else {
            return 0;
        };
        if !min.is_finite() || !max.is_finite() || !size.is_finite() {
            return 0;
        }
        let half = ((size * 0.5 - Vec2::splat(0.201)) / TERRAIN_CELL_SIZE)
            .floor()
            .clamp(Vec2::ZERO, Vec2::splat(200.0));
        let lo = (min / TERRAIN_CELL_SIZE - Vec2::splat(0.5))
            .ceil()
            .clamp(-half, half);
        let hi = (max / TERRAIN_CELL_SIZE - Vec2::splat(0.5))
            .ceil()
            .clamp(-half, half);
        if lo.cmpge(hi).any() || (code == 0 && self.is_empty()) {
            return 0;
        }
        self.cells.resize(SIDE * SIDE, 0);
        let mut changed = 0;
        for y in lo.y as i32..hi.y as i32 {
            for x in lo.x as i32..hi.x as i32 {
                let cell = &mut self.cells[(y + OFFSET) as usize * SIDE + (x + OFFSET) as usize];
                changed += usize::from(*cell != code as u8);
                *cell = code as u8;
            }
        }
        if code == 0 {
            self.compact_empty();
        }
        changed
    }

    fn compact_empty(&mut self) {
        if self.cells.iter().all(|&c| c == 0) {
            self.cells.clear();
        }
    }

    pub fn crop(&mut self, size: Vec2) {
        if self.is_empty() {
            return;
        }
        let half = ((size * 0.5 - Vec2::splat(0.201)) / TERRAIN_CELL_SIZE).floor();
        for y in 0..SIDE {
            for x in 0..SIDE {
                let p = Vec2::new(x as f32 - OFFSET as f32, y as f32 - OFFSET as f32);
                if p.cmplt(-half).any() || p.cmpge(half).any() {
                    self.cells[y * SIDE + x] = 0;
                }
            }
        }
        self.compact_empty();
    }

    /// Derived rectangles feed existing rendering and terrain barriers, never map items.
    pub fn patches(&self) -> Vec<TerrainPatch> {
        let mut result: Vec<TerrainPatch> = vec![];
        let mut previous: BTreeMap<(u8, usize, usize), usize> = BTreeMap::new();
        for (y, row) in self.cells.chunks(SIDE).enumerate() {
            let mut current = BTreeMap::new();
            let mut x = 0;
            while x < SIDE {
                let start = x;
                let code = row[x];
                x += 1;
                while x < SIDE && row[x] == code {
                    x += 1;
                }
                if code == 0 {
                    continue;
                }
                let key = (code, start, x);
                let index = if let Some(&index) = previous.get(&key) {
                    result[index].max.y += TERRAIN_CELL_SIZE;
                    index
                } else {
                    let point = |x: usize, y: usize| {
                        Vec2::new(x as f32 - OFFSET as f32, y as f32 - OFFSET as f32)
                            * TERRAIN_CELL_SIZE
                    };
                    result.push(TerrainPatch {
                        kind: KINDS[code as usize],
                        min: point(start, y),
                        max: point(x, y + 1),
                    });
                    result.len() - 1
                };
                current.insert(key, index);
            }
            previous = current;
        }
        result
    }
}

impl Level {
    /// Import legacy rectangles with their original impassable-terrain precedence.
    pub fn migrate_terrain(&mut self) -> usize {
        let mut count = 0;
        for blocked in [false, true] {
            for item in &self.items {
                if let LevelItem::Terrain { terrain, min, max } = item
                    && terrain.blocks_tanks() == blocked
                {
                    self.terrain_mask.paint(
                        *terrain,
                        (*min).into(),
                        (*max).into(),
                        self.size.into(),
                    );
                    count += 1;
                }
            }
        }
        self.items
            .retain(|item| !matches!(item, LevelItem::Terrain { .. }));
        count
    }

    pub fn paint_terrain(&mut self, kind: TerrainKind, min: Vec2, max: Vec2) {
        self.migrate_terrain();
        self.terrain_mask.paint(kind, min, max, self.size.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_of_strokes_use_no_items_and_roundtrip_as_a_compact_mask() {
        let mut level = Level::default();
        let count = level.items.len();
        for i in 0..5000 {
            let p = Vec2::new(
                (i % 80) as f32 * 0.5 - 20.0,
                (i / 80 % 40) as f32 * 0.5 - 10.0,
            );
            level.paint_terrain(TerrainKind::Sand, p, p + Vec2::splat(0.5));
        }
        assert_eq!(level.items.len(), count);
        assert_eq!(
            level.terrain_mask.at(Vec2::new(-19.75, -9.75)),
            TerrainKind::Sand
        );
        let json = level.to_json().unwrap();
        assert!(json.len() < 10000);
        assert_eq!(level, Level::from_json(&json).unwrap());
        level.paint_terrain(TerrainKind::Ground, Vec2::splat(-100.0), Vec2::splat(100.0));
        assert!(level.terrain_mask.is_empty());
    }

    #[test]
    fn legacy_migration_preserves_water_priority_then_allows_painting_over_it() {
        let mut level = Level::default();
        for terrain in [TerrainKind::Water, TerrainKind::Road] {
            level.items.push(LevelItem::Terrain {
                terrain,
                min: [-2.0, -2.0],
                max: [2.0, 2.0],
            });
        }
        assert_eq!(level.migrate_terrain(), 2);
        assert_eq!(level.terrain_mask.at(Vec2::ZERO), TerrainKind::Water);
        level.paint_terrain(TerrainKind::Sand, Vec2::ZERO, Vec2::splat(0.5));
        assert_eq!(level.terrain_mask.at(Vec2::splat(0.25)), TerrainKind::Sand);
        assert_eq!(
            level.terrain_mask.at(Vec2::splat(-0.25)),
            TerrainKind::Water
        );
        let patches = level.terrain_mask.patches();
        assert_eq!(
            patches
                .iter()
                .map(|p| (p.max.x - p.min.x) * (p.max.y - p.min.y))
                .sum::<f32>(),
            16.0
        );
    }
}
