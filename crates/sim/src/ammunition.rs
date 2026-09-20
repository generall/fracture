//! Weapon profiles in SI units.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ammunition {
    #[default]
    #[serde(alias = "machine_gun")]
    HighExplosive,
    ArmorPiercing,
    Rocket,
    Laser,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialWeapon {
    Laser,
    Rocket,
}
impl SpecialWeapon {
    pub fn ammunition(self) -> Ammunition {
        match self {
            Self::Laser => Ammunition::Laser,
            Self::Rocket => Ammunition::Rocket,
        }
    }
    pub fn label(self) -> &'static str {
        self.ammunition().label()
    }
}
impl Ammunition {
    /// All profiles, including specials, for material experiments.
    pub const ALL: [Self; 4] = [
        Self::HighExplosive,
        Self::ArmorPiercing,
        Self::Rocket,
        Self::Laser,
    ];
    pub const MAIN: [Self; 2] = [Self::HighExplosive, Self::ArmorPiercing];
    pub fn next(self) -> Self {
        if self == Self::HighExplosive {
            Self::ArmorPiercing
        } else {
            Self::HighExplosive
        }
    }
    pub fn special(self) -> bool {
        matches!(self, Self::Rocket | Self::Laser)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::HighExplosive => "HE",
            Self::ArmorPiercing => "AP",
            Self::Rocket => "ROCKET",
            Self::Laser => "LASER",
        }
    }
    pub fn defaults(self) -> (f32, f32) {
        match self {
            Self::HighExplosive => (6.0, 220.0),
            Self::ArmorPiercing => (8.0, 350.0),
            Self::Rocket => (16.0, 12.0),
            Self::Laser => (0.0, 0.0),
        }
    }
    pub fn mass_range(self) -> (f32, f32) {
        match self {
            Self::Laser => (0.0, 20.0),
            _ => (1.0, 20.0),
        }
    }
    pub fn speed_range(self) -> (f32, f32) {
        match self {
            Self::Rocket => (8.0, 20.0),
            Self::Laser => (0.0, 900.0),
            _ => (80.0, 400.0),
        }
    }
    pub fn interval(self) -> f32 {
        if self == Self::Laser { 0.1 } else { 0.9 }
    }
    pub fn automatic(self) -> bool {
        self == Self::Laser
    }
    pub(crate) fn radius(self) -> f32 {
        if self == Self::Rocket { 0.12 } else { 0.035 }
    }
}
