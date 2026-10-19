//! Playable vehicle profiles. Geometry, controls and sight share these values.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleKind {
    Scout,
    #[default]
    Tank,
    Mortar,
}
impl VehicleKind {
    pub const ALL: [Self; 3] = [Self::Scout, Self::Tank, Self::Mortar];
    pub fn label(self) -> &'static str {
        match self {
            Self::Scout => "SCOUT CAR",
            Self::Tank => "REGULAR TANK",
            Self::Mortar => "MORTAR",
        }
    }
    pub fn speed(self) -> f32 {
        match self {
            Self::Scout => 10.0,
            Self::Tank => 6.5,
            Self::Mortar => 3.6,
        }
    }
    pub fn mass(self) -> f32 {
        match self {
            Self::Scout => 1200.0,
            Self::Tank => 4500.0,
            Self::Mortar => 16000.0,
        }
    }
    pub fn size(self) -> crate::Vec2 {
        match self {
            Self::Scout => crate::Vec2::new(0.82, 0.8),
            Self::Tank => crate::Vec2::ONE,
            Self::Mortar => crate::Vec2::new(1.22, 1.12),
        }
    }
    pub fn reload(self) -> f32 {
        match self {
            Self::Scout => 0.18,
            Self::Tank => 0.9,
            Self::Mortar => 3.0,
        }
    }
    pub fn sight_multiplier(self) -> f32 {
        if self == Self::Mortar { 1.5 } else { 1.0 }
    }
    pub fn near_vision(self) -> f32 {
        if self == Self::Mortar { 6.0 } else { 3.0 }
    }
    pub fn vision_half_angle(self) -> f32 {
        match self {
            Self::Scout => std::f32::consts::PI,
            Self::Tank => std::f32::consts::FRAC_PI_4,
            Self::Mortar => 22.5_f32.to_radians(),
        }
    }
}
