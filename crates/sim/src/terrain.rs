//! Horizontal terrain and a deliberately small 2.5D bridge model.
use crate::{BodyView, Vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TerrainKind {
    #[default]
    Ground,
    Road,
    Sand,
    Water,
    Cliff,
    Bridge,
    TallGrass,
    Elevation,
}

impl TerrainKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ground => "GROUND",
            Self::Road => "ROAD",
            Self::Sand => "SAND",
            Self::TallGrass => "TALL GRASS",
            Self::Elevation => "ELEVATION",
            Self::Water => "WATER",
            Self::Cliff => "CLIFF",
            Self::Bridge => "BRIDGE",
        }
    }

    /// Impassable terrain; only an intact bridge supplies a crossing.
    pub fn blocks_tanks(self) -> bool {
        matches!(self, Self::Water | Self::Cliff)
    }

    /// Speed, drive force, grip and lateral damping relative to ordinary ground.
    pub(crate) fn traction(self) -> [f32; 4] {
        match self {
            Self::Ground | Self::TallGrass | Self::Elevation => [1.0; 4],
            Self::Road | Self::Bridge => [1.45, 1.3, 1.4, 1.35],
            Self::Sand => [0.48, 0.6, 0.65, 0.8],
            Self::Water | Self::Cliff => [0.0; 4],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPatch {
    pub kind: TerrainKind,
    pub min: Vec2,
    pub max: Vec2,
}

impl TerrainPatch {
    pub fn contains(self, point: Vec2) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    /// Rectangle subtraction used to cut physical bridge openings out of water.
    pub(crate) fn subtract(self, opening: Self) -> Vec<Self> {
        let lo = self.min.max(opening.min);
        let hi = self.max.min(opening.max);
        if lo.cmpge(hi).any() {
            return vec![self];
        }
        [
            (self.min, Vec2::new(lo.x, self.max.y)),
            (Vec2::new(hi.x, self.min.y), self.max),
            (Vec2::new(lo.x, self.min.y), Vec2::new(hi.x, lo.y)),
            (Vec2::new(lo.x, hi.y), Vec2::new(hi.x, self.max.y)),
        ]
        .into_iter()
        .filter(|(min, max)| (max - min).cmpgt(Vec2::splat(1e-5)).all())
        .map(|(min, max)| Self { min, max, ..self })
        .collect()
    }
}

#[derive(Clone, Copy)]
pub struct BridgeView<'a> {
    pub deck: TerrainPatch,
    pub controls: [Vec2; 2],
    pub intact: bool,
    /// Zero at failure, one when the span has disappeared under water.
    pub fall_progress: f32,
    pub fragments: &'a [BodyView],
}
