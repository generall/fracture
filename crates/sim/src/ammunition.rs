//! Effective weapon profiles in SI units. Object damage still comes from
//! material stress/work, except the laser's explicit, force-free removal.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ammunition {
    #[default]
    HighExplosive,
    ArmorPiercing,
    MachineGun,
    Laser,
}

impl Ammunition {
    pub const ALL: [Self; 4] = [
        Self::HighExplosive,
        Self::ArmorPiercing,
        Self::MachineGun,
        Self::Laser,
    ];
    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::HighExplosive => "HE",
            Self::ArmorPiercing => "AP",
            Self::MachineGun => "MG",
            Self::Laser => "LASER",
        }
    }
    pub fn defaults(self) -> (f32, f32) {
        match self {
            Self::HighExplosive => (6.0, 220.0),
            Self::ArmorPiercing => (8.0, 350.0),
            Self::MachineGun => (0.024, 850.0),
            Self::Laser => (0.0, 0.0),
        }
    }
    pub fn mass_range(self) -> (f32, f32) {
        match self {
            Self::MachineGun => (0.002, 0.03),
            Self::Laser => (0.0, 20.0), // Ignored; legacy test fields can remain.
            _ => (1.0, 20.0),
        }
    }
    pub fn speed_range(self) -> (f32, f32) {
        match self {
            Self::MachineGun => (150.0, 900.0),
            Self::Laser => (0.0, 900.0),
            _ => (80.0, 400.0),
        }
    }
    pub fn interval(self) -> f32 {
        match self {
            Self::HighExplosive | Self::ArmorPiercing => 0.45,
            Self::MachineGun => 1.0 / 12.0,
            Self::Laser => 1.0 / 6.0,
        }
    }
    pub fn automatic(self) -> bool {
        matches!(self, Self::MachineGun | Self::Laser)
    }
    pub(crate) fn radius(self) -> f32 {
        if self == Self::MachineGun {
            0.004
        } else {
            0.035
        }
    }
}
