//! Starter components and object recipes. These are tunable game materials,
//! not measured barrel, rock or masonry constitutive models.
use super::*;

/// Effective plastic response of a thin/hollow game object. Stress is in Pa;
/// flow is an overstress strain rate, hardening is dimensionless.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plasticity {
    pub yield_stress: f32,
    pub flow_rate: f32,
    pub hardening: f32,
    pub max_strain: f32,
}
impl Default for Plasticity {
    fn default() -> Self {
        Self {
            yield_stress: 1000.0,
            flow_rate: 2.0,
            hardening: 2.0,
            max_strain: 1.2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crushable {
    /// Opposed contact force in N needed to activate shell buckling.
    pub force: f32,
    /// Strength and fracture-work multiplier while squeezed, relative to
    /// ordinary contact resistance. No work or impulse is created by this rule.
    pub resistance: f32,
}
impl Default for Crushable {
    fn default() -> Self {
        Self {
            force: 2500.0,
            resistance: 0.0001,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explosive {
    pub energy: f32,
    pub radius: f32,
}
impl Default for Explosive {
    fn default() -> Self {
        Self {
            energy: 600_000.0,
            radius: 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Components {
    /// Plastic yield moment between material grains, in N m.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bending_torque: Option<f32>,
    /// None uses the material/preset default; independent of physical collision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_vision: Option<bool>,
    /// Phenomenological strength/work multipliers; actual impulse is unchanged.
    pub projectile_resistance: f32,
    pub contact_resistance: f32,
    pub contact_refinement: bool,
    pub crush: Option<Crushable>,
    /// Ignites once, on the first accepted plastic deformation or seam fracture.
    pub explosive: Option<Explosive>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plasticity: Option<Plasticity>,
}
impl Default for Components {
    fn default() -> Self {
        Self {
            blocks_vision: None,
            bending_torque: None,
            projectile_resistance: 1.0,
            contact_resistance: 1.0,
            contact_refinement: true,
            crush: None,
            explosive: None,
            plasticity: None,
        }
    }
}
impl Components {
    pub(super) fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .bending_torque
            .is_some_and(|t| !t.is_finite() || !(0.1..=100000.0).contains(&t))
        {
            return Err("Bending torque must be between 0.1 and 100000 N m.".into());
        }
        if let Some(p) = self.plasticity {
            for (name, value, min, max) in [
                ("Yield stress", p.yield_stress, 100.0, 1e9),
                ("Plastic flow", p.flow_rate, 0.01, 20.0),
                ("Hardening", p.hardening, 0.0, 100.0),
                ("Plastic strain limit", p.max_strain, 0.02, 3.0),
            ] {
                if !value.is_finite() || !(min..=max).contains(&value) {
                    return Err(format!("{name} must be between {min} and {max}."));
                }
            }
        }
        for (name, value, min, max) in [
            (
                "Projectile resistance",
                self.projectile_resistance,
                0.1,
                1000.0,
            ),
            ("Contact resistance", self.contact_resistance, 0.1, 1000.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{name} must be between {min} and {max}."));
            }
        }
        if let Some(c) = self.crush
            && (!c.force.is_finite()
                || !(100.0..=1e6).contains(&c.force)
                || !c.resistance.is_finite()
                || !(0.0001..=1.0).contains(&c.resistance))
        {
            return Err("Crush force must be 100–1000000 N; crush resistance 0.0001–1.".into());
        }
        if let Some(e) = self.explosive
            && (!e.energy.is_finite()
                || !(1000.0..=5_000_000.0).contains(&e.energy)
                || !e.radius.is_finite()
                || !(0.25..=8.0).contains(&e.radius))
        {
            return Err("Explosion must use 1–5000 kJ and a radius of 0.25–8 m.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LibraryPreset {
    Barrel,
    ExplosiveBarrel,
    Wall,
    Rock,
    MetalFence,
}
impl LibraryPreset {
    pub const ALL: [Self; 5] = [
        Self::Barrel,
        Self::ExplosiveBarrel,
        Self::Wall,
        Self::Rock,
        Self::MetalFence,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Barrel => "BARREL",
            Self::ExplosiveBarrel => "EXPLOSIVE BARREL",
            Self::Wall => "WALL",
            Self::Rock => "ROCK",
            Self::MetalFence => "METAL FENCE",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Barrel => "Light and mobile. Resists shells; buckles when squeezed.",
            Self::ExplosiveBarrel => "Permanent damage ignites a blast that can hit its neighbors.",
            Self::Wall => "Anchored masonry. Heavy chunks; finer shell-impact detail.",
            Self::Rock => "Ground-anchored core. Shells chip stone; tanks cannot push it.",
            Self::MetalFence => "Anchored metal strip. Bends permanently under a tank load.",
        }
    }
    pub fn asset(self) -> ObjectAsset {
        let mut a = ObjectAsset::workshop_preset(match self {
            Self::Wall => ObjectPreset::Wall,
            Self::Rock => ObjectPreset::Rock,
            _ => ObjectPreset::Plate,
        });
        a.name = self.label().to_ascii_lowercase();
        match self {
            Self::MetalFence => {
                a.outline = vec![[-0.1, -2.5], [0.1, -2.5], [0.1, 2.5], [-0.1, 2.5]];
                a.surface = Surface::Panel;
                a.density = 780.0;
                a.thickness = 0.02;
                a.friction = 0.02;
                a.tension = 350e6;
                a.compression = 350e6;
                a.shear = 200e6;
                a.young = Some(200e9);
                a.toughness = Some(400.0);
                a.cohesion = 4.0;
                a.refinement = 0;
                a.pinned_edge = Some(0);
                // Accepted fence trial #3: six plastic bend points, with the
                // same pinned base and free-span layout as the comparison.
                let spacing = 3.6 / 6.0;
                a.layout = Some(GrainLayout {
                    mode: GrainMode::Stones,
                    size: [0.2, spacing],
                    seed: 7,
                    sites: std::iter::once([0.0, -1.1 - spacing * 0.5])
                        .chain((0..6).map(|j| [0.0, -1.1 + (j as f32 + 0.5) * spacing]))
                        .collect(),
                });
                a.components = Components {
                    contact_refinement: false,
                    bending_torque: Some(8000.0),
                    ..Components::default()
                };
                a.color = Some([110, 184, 255]);
            }
            Self::Barrel | Self::ExplosiveBarrel => {
                a.circular = true;
                a.outline = (0..16)
                    .map(|i| {
                        let t = i as f32 * std::f32::consts::TAU / 16.0;
                        [t.cos() * 0.65, t.sin() * 0.65]
                    })
                    .collect();
                a.surface = Surface::Panel;
                a.density = 140.0;
                a.thickness = 0.4;
                a.friction = 0.09;
                if matches!(self, Self::Barrel) {
                    a.soft_bounce = Some(SoftBounce {
                        restitution: 0.62, // Selected wall-release case 2.
                        ..SoftBounce::default()
                    });
                }
                a.tension = 800_000.0;
                a.compression = 1_600_000.0;
                a.shear = 800_000.0;
                a.young = Some(12e9);
                a.toughness = Some(40.0);
                a.cohesion = 1.0;
                a.refinement = 0;
                a.pinned_edge = None;
                a.layout = Some(GrainLayout {
                    mode: GrainMode::Stones,
                    size: [1.3, 1.3],
                    seed: 7,
                    sites: vec![[0.0, 0.0]],
                });
                a.components = Components {
                    blocks_vision: Some(false),
                    bending_torque: None,
                    projectile_resistance: 60.0,
                    contact_resistance: 8.0,
                    contact_refinement: false,
                    crush: Some(Crushable::default()),
                    explosive: None,
                    plasticity: Some(if matches!(self, Self::Barrel) {
                        // Accepted deformation-study case 6.
                        Plasticity {
                            yield_stress: 500.0,
                            flow_rate: 2.0,
                            hardening: 0.0,
                            max_strain: 3.0,
                        }
                    } else {
                        Plasticity::default()
                    }),
                };
                a.color = Some([105, 224, 174]);
                if matches!(self, Self::ExplosiveBarrel) {
                    a.components.projectile_resistance = 3.0;
                    a.components.explosive = Some(Explosive::default());
                    a.color = Some([255, 108, 96]);
                }
            }
            Self::Wall => {
                a.density = 2400.0;
                a.thickness = 0.55;
                a.friction = 1.3;
                // Ordinary masonry seams and the original single support:
                // shells make rubble; tank impacts separate larger blocks.
                a.cohesion = 4.0;
                a.toughness = Some(55.0);
                a.components.contact_resistance = 1.5;
                a.components.contact_refinement = false;
                a.regenerate_grains(GrainMode::Bricks, [0.7, 0.6], 7);
                a.color = Some([243, 198, 110]);
            }
            Self::Rock => {
                a.outline = vec![
                    [-1.35, -0.55],
                    [-0.95, -1.15],
                    [-0.30, -1.30],
                    [0.45, -1.06],
                    [1.10, -0.65],
                    [1.36, 0.03],
                    [0.95, 0.88],
                    [0.33, 1.17],
                    [-0.40, 1.02],
                    [-1.22, 0.65],
                ];
                a.density = 2900.0;
                a.thickness = 0.65;
                a.friction = 1.1;
                // Start with the previous wall's projectile-facing seam profile.
                let strength = Material::Masonry.strength();
                a.tension = strength.tension;
                a.compression = strength.compression;
                a.shear = strength.shear;
                a.young = Some(15e9);
                a.toughness = Some(55.0);
                a.cohesion = 18.0;
                a.ground_anchor = true;
                a.components.contact_resistance = 1000.0;
                a.components.contact_refinement = false;
                a.regenerate_grains(GrainMode::Stones, [0.9, 0.75], 31);
                a.color = Some([171, 161, 240]);
            }
        }
        a
    }
}

/// User-authored copies are separate from the immutable starter recipes.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectLibrary {
    pub objects: Vec<ObjectAsset>,
}
impl ObjectLibrary {
    pub const MAX_BYTES: usize = 1_048_576;
    pub fn from_json(s: &str) -> Result<Self, String> {
        if s.len() > Self::MAX_BYTES {
            return Err("Object library exceeds 1 MiB.".into());
        }
        let l: Self = serde_json::from_str(s).map_err(|e| e.to_string())?;
        l.validate()?;
        Ok(l)
    }
    fn validate(&self) -> Result<(), String> {
        if self.objects.len() > 24 {
            return Err("Library holds 24 custom objects. Export or remove an older copy.".into());
        }
        for a in &self.objects {
            a.mesh()?;
        }
        Ok(())
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let s = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if s.len() > Self::MAX_BYTES {
            return Err("Object library exceeds 1 MiB.".into());
        }
        Ok(s)
    }
    pub fn save(&mut self, asset: ObjectAsset) {
        if let Some(i) = self.objects.iter().position(|a| a.name == asset.name) {
            self.objects[i] = asset;
        } else {
            self.objects.push(asset);
        }
    }
}
