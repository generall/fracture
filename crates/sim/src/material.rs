/// Deliberately approximate material parameters for the laboratory, not measured
/// engineering data. Strength describes a seam, rather than the solid brick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Material {
    Masonry,
    Panel,
    Rock,
    Anchor,
    Projectile,
    Launcher,
    Boundary,
}

impl Material {
    pub(crate) fn density(self) -> f32 {
        match self {
            Self::Masonry => 1_600.0,
            Self::Panel => 850.0,
            Self::Rock => 2_600.0,
            _ => 2_000.0,
        }
    }

    pub(crate) fn strength(self) -> Strength {
        match self {
            Self::Rock => Strength {
                tension: 260_000.0,
                compression: 8_000_000.0,
                shear: 380_000.0,
            },
            Self::Panel => Strength {
                tension: 70_000.0,
                compression: 350_000.0,
                shear: 50_000.0,
            },
            _ => Strength {
                tension: 88_000.0,
                compression: 400_000.0,
                shear: 64_000.0,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Strength {
    pub tension: f32,
    pub compression: f32,
    pub shear: f32,
}

impl Strength {
    /// Rectangular seam of in-plane length `width` and out-of-plane `depth`.
    /// Positive normal force is tension. Bending adds opposing edge stresses.
    pub fn utilization(self, normal: f32, shear: f32, torque: f32, width: f32, depth: f32) -> f32 {
        let area = width * depth;
        let axial = normal / area;
        let bending = torque.abs() / (depth * width * width / 6.0);
        let tension = (axial + bending).max(0.0) / self.tension;
        let compression = (-axial + bending).max(0.0) / self.compression;
        tension
            .max(compression)
            .max(shear.abs() / area / self.shear)
    }
}
