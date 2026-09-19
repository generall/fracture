//! Portable maps. Instances embed independent assets rather than mutable library references.
use crate::{LibraryPreset, ObjectAsset, ObjectMesh, TerrainKind, Vec2};
use serde::{Deserialize, Serialize};
mod authoring;
mod composition;
pub use authoring::{clamp_translation, merge_items};

pub const MAX_LEVEL_BYTES: usize = 2 * 1024 * 1024;
pub const FLAG_RADIUS: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Team {
    Blue,
    Red,
}
impl Team {
    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "BLUE",
            Self::Red => "RED",
        }
    }
    pub fn other(self) -> Self {
        match self {
            Self::Blue => Self::Red,
            Self::Red => Self::Blue,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LevelItem {
    Object {
        asset: Box<ObjectAsset>,
        position: [f32; 2],
        angle: f32,
    },
    /// Fixed polygon: blocks tanks and weapons, never fractures.
    Solid {
        outline: Vec<[f32; 2]>,
        position: [f32; 2],
        angle: f32,
    },
    Terrain {
        terrain: TerrainKind,
        min: [f32; 2],
        max: [f32; 2],
    },
    /// Rectangle is the crossing surface; booths sit alongside its two banks.
    Bridge {
        min: [f32; 2],
        max: [f32; 2],
        vertical: bool,
    },
    Spawn {
        team: Team,
        position: [f32; 2],
        angle: f32,
    },
    Flag {
        team: Team,
        position: [f32; 2],
    },
}

/// Behavior of the opposing tank when playtesting either team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyActivity {
    #[default]
    Static,
    Driving,
    Attack,
    Conquer,
}
impl EnemyActivity {
    pub const ALL: [Self; 4] = [Self::Static, Self::Driving, Self::Attack, Self::Conquer];
    pub fn label(self) -> &'static str {
        match self {
            Self::Static => "1 / STATIC",
            Self::Driving => "2 / DRIVING",
            Self::Attack => "3 / ATTACK",
            Self::Conquer => "4 / CONQUER",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Static => "Stays still and never fires. Use it as a damage target.",
            Self::Driving => {
                "Patrols a route. Avoids heavy obstacles; may ram light walls. Never fires."
            }
            Self::Attack => {
                "Hunts the player with delayed reactions, limited turret speed and imperfect aim."
            }
            Self::Conquer => "Heads for your flag and fires at you when nearby.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub version: u32,
    pub name: String,
    /// Arena dimensions in metres; origin at its centre.
    pub size: [f32; 2],
    pub items: Vec<LevelItem>,
    #[serde(default)]
    pub enemy_activity: EnemyActivity,
}
impl Default for Level {
    fn default() -> Self {
        Self {
            version: 1,
            enemy_activity: EnemyActivity::Static,
            name: "Untitled level".into(),
            size: [48.0, 30.0],
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-18.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Spawn {
                    team: Team::Red,
                    position: [18.0, 0.0],
                    angle: std::f32::consts::PI,
                },
                LevelItem::Flag {
                    team: Team::Blue,
                    position: [-21.0, 0.0],
                },
                LevelItem::Flag {
                    team: Team::Red,
                    position: [21.0, 0.0],
                },
            ],
        }
    }
}

impl Level {
    /// A bundled, fully editable map. Its JSON is also available for download.
    pub fn demo() -> Self {
        Self::from_json(include_str!("../../../web/levels/quarry-crossing.json"))
            .expect("bundled Quarry Crossing map is valid")
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_LEVEL_BYTES {
            return Err("Level file exceeds 2 MiB.".into());
        }
        let level: Self =
            serde_json::from_str(json).map_err(|e| format!("Invalid level JSON: {e}"))?;
        level.validate()?;
        Ok(level)
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if json.len() > MAX_LEVEL_BYTES {
            return Err("Level file exceeds 2 MiB.".into());
        }
        Ok(json)
    }
    /// Drafts may omit objectives/spawns. A playable team requires a clear spawn.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported level version; expected 1.".into());
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 48
            || self.name.chars().any(char::is_control)
        {
            return Err("Level name must contain 1–48 printable characters.".into());
        }
        if self
            .size
            .iter()
            .any(|n| !n.is_finite() || !(12.0..=200.0).contains(n))
        {
            return Err("Map dimensions must be 12–200 metres.".into());
        }
        if self.items.len() > 128 {
            return Err("Use at most 128 map items.".into());
        }
        let mut pieces = 5;
        let mut bridges = 0;
        for (i, item) in self.items.iter().enumerate() {
            let check = || -> Result<usize, String> {
                if !item.angle().is_finite() {
                    return Err("Rotation must be finite.".into());
                }
                let vertices = item.world_outline();
                if vertices.iter().any(|p| {
                    !p.is_finite()
                        || p.abs()
                            .cmpgt(Vec2::from(self.size) * 0.5 - Vec2::splat(0.2))
                            .any()
                }) {
                    return Err("Placement extends beyond the map boundary.".into());
                }
                match item {
                    LevelItem::Object { asset, .. } => {
                        Ok(asset.level_mesh()?.triangles.len() + asset.support_segments().len())
                    }
                    LevelItem::Solid { outline, .. } => {
                        Ok(solid_asset(outline).level_mesh()?.triangles.len())
                    }
                    LevelItem::Terrain { terrain, min, max } => {
                        if *terrain == TerrainKind::Bridge {
                            return Err("Use a bridge item for a supported crossing.".into());
                        }
                        valid_rect(*min, *max)?;
                        Ok(0)
                    }
                    LevelItem::Bridge { min, max, vertical } => {
                        valid_rect(*min, *max)?;
                        Ok(bridge_mesh(Vec2::from(*max) - Vec2::from(*min), *vertical)?
                            .triangles
                            .len()
                            + 2)
                    }
                    LevelItem::Flag { team, .. } | LevelItem::Spawn { team, .. } => {
                        if self.items[..i].iter().any(|other| {
                            matches!((item, other),
                            (LevelItem::Flag {..}, LevelItem::Flag {team: t, ..}) |
                            (LevelItem::Spawn {..}, LevelItem::Spawn {team: t, ..}) if t == team)
                        }) {
                            return Err("Use one spawn and one flag per team.".into());
                        }
                        Ok(0)
                    }
                }
            };
            pieces += check().map_err(|e| format!("Item {} ({}): {e}", i + 1, item.label()))?;
            if matches!(item, LevelItem::Bridge { .. }) {
                bridges += 1;
            }
        }
        if bridges > 8 {
            return Err("Use at most eight bridges.".into());
        }
        if pieces > 8192 {
            return Err(
                "Level exceeds the 8192-piece budget; use fewer objects or coarser grains.".into(),
            );
        }
        Ok(())
    }
}

fn valid_rect(min: [f32; 2], max: [f32; 2]) -> Result<(), String> {
    if (Vec2::from(max) - Vec2::from(min))
        .cmplt(Vec2::splat(0.3))
        .any()
    {
        return Err("Rectangle dimensions must be at least 0.3 m.".into());
    }
    Ok(())
}

impl LevelItem {
    pub fn label(&self) -> &str {
        match self {
            Self::Object { asset, .. } => &asset.name,
            Self::Solid { .. } => "UNBREAKABLE WALL",
            Self::Terrain { terrain, .. } => terrain.label(),
            Self::Bridge { .. } => "BRIDGE",
            Self::Spawn {
                team: Team::Blue, ..
            } => "BLUE SPAWN",
            Self::Spawn { .. } => "RED SPAWN",
            Self::Flag {
                team: Team::Blue, ..
            } => "BLUE FLAG",
            Self::Flag { .. } => "RED FLAG",
        }
    }
    pub fn position(&self) -> Vec2 {
        match self {
            Self::Object { position, .. }
            | Self::Solid { position, .. }
            | Self::Spawn { position, .. }
            | Self::Flag { position, .. } => Vec2::from(*position),
            Self::Terrain { min, max, .. } | Self::Bridge { min, max, .. } => {
                (Vec2::from(*min) + Vec2::from(*max)) * 0.5
            }
        }
    }
    pub fn angle(&self) -> f32 {
        match self {
            Self::Object { angle, .. } | Self::Solid { angle, .. } | Self::Spawn { angle, .. } => {
                *angle
            }
            _ => 0.0,
        }
    }
    pub fn translate(&mut self, delta: Vec2) {
        match self {
            Self::Object { position, .. }
            | Self::Solid { position, .. }
            | Self::Spawn { position, .. }
            | Self::Flag { position, .. } => *position = (Vec2::from(*position) + delta).to_array(),
            Self::Terrain { min, max, .. } | Self::Bridge { min, max, .. } => {
                *min = (Vec2::from(*min) + delta).to_array();
                *max = (Vec2::from(*max) + delta).to_array();
            }
        }
    }
    pub fn local_outline(&self) -> Vec<[f32; 2]> {
        match self {
            Self::Object { asset, .. } => asset.outline.clone(),
            Self::Solid { outline, .. } => outline.clone(),
            Self::Terrain { min, max, .. } | Self::Bridge { min, max, .. } => {
                rectangle((Vec2::from(*max) - Vec2::from(*min)) * 0.5)
            }
            Self::Spawn { .. } => rectangle(Vec2::new(1.08, 0.82)),
            Self::Flag { .. } => rectangle(Vec2::splat(FLAG_RADIUS)),
        }
    }
    pub fn world_outline(&self) -> Vec<Vec2> {
        let mut points = self.local_outline();
        if let Self::Bridge { min, max, vertical } = self
            && let Ok(asset) = bridge_asset(Vec2::from(*max) - Vec2::from(*min), *vertical)
        {
            points = asset.outline;
        }
        let r = Vec2::from_angle(self.angle());
        points
            .into_iter()
            .map(|p| self.position() + r.rotate(Vec2::from(p)))
            .collect()
    }
    pub fn dimensions(&self) -> Vec2 {
        let p = self.local_outline();
        let lo = p
            .iter()
            .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(Vec2::from(*p)));
        let hi = p
            .iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(Vec2::from(*p)));
        hi - lo
    }
    /// Regenerate grains at physical dimensions instead of stretching stone/brick cells.
    pub fn reshape(&mut self, outline: Vec<[f32; 2]>) {
        match self {
            Self::Object { asset, .. } => {
                if asset.is_circular() {
                    return;
                }
                asset.outline = outline;
                if let Some(layout) = &asset.layout {
                    asset.regenerate_grains(layout.mode, layout.size, layout.seed);
                }
                asset.fit_grain_budget();
            }
            Self::Solid {
                outline: current, ..
            } => *current = outline,
            _ => {}
        }
    }
    pub fn resize(&mut self, dimensions: Vec2) -> Result<(), String> {
        if !dimensions.is_finite() {
            return Err("Dimensions must be finite.".into());
        }
        let mut dimensions = dimensions.clamp(Vec2::splat(0.3), Vec2::splat(190.0));
        if let Self::Bridge { vertical, .. } = self {
            dimensions = dimensions.max(if *vertical {
                Vec2::new(2.0, 3.0)
            } else {
                Vec2::new(3.0, 2.0)
            });
        }
        if let Self::Object { asset, .. } = self
            && asset.is_circular()
        {
            let old = self.dimensions();
            let diameter = if (dimensions.x - old.x).abs() >= (dimensions.y - old.y).abs() {
                dimensions.x
            } else {
                dimensions.y
            };
            if let Self::Object { asset, .. } = self {
                asset.set_radius(diameter * 0.5);
            }
            return Ok(());
        }
        match self {
            Self::Terrain { min, max, .. } | Self::Bridge { min, max, .. } => {
                let center = (Vec2::from(*min) + Vec2::from(*max)) * 0.5;
                *min = (center - dimensions * 0.5).to_array();
                *max = (center + dimensions * 0.5).to_array();
            }
            Self::Object { .. } | Self::Solid { .. } => {
                let ratio = dimensions / self.dimensions();
                self.reshape(
                    self.local_outline()
                        .iter()
                        .map(|p| (Vec2::from(*p) * ratio).to_array())
                        .collect(),
                );
            }
            _ => return Err("Spawn and flag footprints have a fixed size.".into()),
        }
        Ok(())
    }
}
pub fn rectangle(half: Vec2) -> Vec<[f32; 2]> {
    vec![
        [-half.x, -half.y],
        [half.x, -half.y],
        [half.x, half.y],
        [-half.x, half.y],
    ]
}

/// A drawn wall without generated fixtures. Authored solid obstacles provide
/// nearby breakable attachments when the level is instantiated.
pub fn wall_asset(dimensions: Vec2) -> ObjectAsset {
    let mut asset = LibraryPreset::Wall.asset();
    asset.outline = rectangle(dimensions * 0.5);
    asset.pinned_edge = None;
    asset.additional_pinned_edges.clear();
    asset.support_spacing = None;
    asset.regenerate_grains(crate::GrainMode::Bricks, [0.7, 0.6], 7);
    asset.fit_grain_budget();
    asset
}

pub(crate) fn solid_asset(outline: &[[f32; 2]]) -> ObjectAsset {
    let mut asset = LibraryPreset::Wall.asset();
    asset.outline = outline.to_vec();
    asset.layout = None;
    asset.refinement = 0;
    asset.pinned_edge = None;
    asset.additional_pinned_edges.clear();
    asset
}

/// Same reinforced booth / untargetable deck model as the terrain range.
pub(crate) fn bridge_asset(size: Vec2, vertical: bool) -> Result<ObjectAsset, String> {
    let size = if vertical {
        Vec2::new(size.y, size.x)
    } else {
        size
    };
    if !size.is_finite() || !(3.0..=190.0).contains(&size.x) || !(2.0..=190.0).contains(&size.y) {
        return Err("Bridge span must be 3–190 m and lane width 2–190 m.".into());
    }
    let (x, y) = (size.x * 0.5, size.y * 0.5);
    let mut a = LibraryPreset::Wall.asset();
    a.name = "Bridge".into();
    a.outline = vec![
        [-x, -y],
        [x, -y],
        [x, y + 1.1],
        [x - 0.8, y + 1.1],
        [x - 0.8, y],
        [-x + 0.8, y],
        [-x + 0.8, y + 1.1],
        [-x, y + 1.1],
    ];
    a.pinned_edge = Some(2);
    a.additional_pinned_edges = vec![6];
    a.thickness = 0.3;
    a.components.contact_resistance = 1000.0;
    a.color = Some([218, 223, 191]);
    a.layout = None;
    a.refinement = 0;
    if vertical {
        for p in &mut a.outline {
            *p = [-p[1], p[0]];
        }
    }
    Ok(a)
}

/// A conforming grid with explicit deck/booth edges. The fixed-size booths
/// retain their targetable cells even when the crossing is hundreds of metres.
pub(crate) fn bridge_mesh(size: Vec2, vertical: bool) -> Result<ObjectMesh, String> {
    bridge_asset(size, vertical)?;
    let size = if vertical {
        Vec2::new(size.y, size.x)
    } else {
        size
    };
    let (x, y) = (size.x * 0.5, size.y * 0.5);
    let nx = ((size.x - 1.6) / 0.8).ceil().clamp(1.0, 16.0) as usize;
    let ny = (size.y / 0.55).ceil().clamp(1.0, 6.0) as usize;
    let mut xs = vec![-x, -x + 0.4];
    xs.extend((0..=nx).map(|i| -x + 0.8 + (size.x - 1.6) * i as f32 / nx as f32));
    xs[2 + nx] = x - 0.8;
    xs.extend([x - 0.4, x]);
    let mut ys: Vec<_> = (0..=ny)
        .map(|i| -y + size.y * i as f32 / ny as f32)
        .collect();
    ys[ny] = y;
    ys.extend([y + 0.55, y + 1.1]);
    let mut mesh = ObjectMesh {
        vertices: vec![],
        triangles: vec![],
        grains: vec![],
        area: size.x * size.y + 1.76,
    };
    for &py in &ys {
        for &px in &xs {
            mesh.vertices.push(if vertical {
                Vec2::new(-py, px)
            } else {
                Vec2::new(px, py)
            });
        }
    }
    for row in 0..ys.len() - 1 {
        for col in 0..xs.len() - 1 {
            if row >= ny && col >= 2 && col < xs.len() - 3 {
                continue;
            }
            let a = row * xs.len() + col;
            let b = a + 1;
            let c = b + xs.len();
            let d = a + xs.len();
            let grain = mesh.grains.len() / 2;
            mesh.triangles.extend([[a, b, c], [a, c, d]]);
            mesh.grains.extend([grain; 2]);
        }
    }
    Ok(mesh)
}

pub(crate) fn rotate_asset_mesh(asset: &mut ObjectAsset, mesh: &mut ObjectMesh, angle: f32) {
    let r = Vec2::from_angle(angle);
    for p in &mut asset.outline {
        *p = r.rotate(Vec2::from(*p)).to_array();
    }
    for p in &mut mesh.vertices {
        *p = r.rotate(*p);
    }
}
