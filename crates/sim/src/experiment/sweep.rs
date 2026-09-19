//! Parameter sweeps transform recipes; running them belongs to `Runner`.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sweep {
    Strength,
    Modulus,
    Density,
    GrainSize,
    FractureWork,
    ShellMass,
    ShellSpeed,
    TankMass,
    TankSpeed,
    YieldStress,
    PlasticFlow,
    Hardening,
    StrainLimit,
}
impl Sweep {
    pub const PLASTIC: [Self; 11] = [
        Self::StrainLimit,
        Self::YieldStress,
        Self::PlasticFlow,
        Self::Hardening,
        Self::Modulus,
        Self::Density,
        Self::GrainSize,
        Self::ShellMass,
        Self::ShellSpeed,
        Self::TankMass,
        Self::TankSpeed,
    ];
    pub const ALL: [Self; 9] = [
        Self::Strength,
        Self::Modulus,
        Self::Density,
        Self::GrainSize,
        Self::FractureWork,
        Self::ShellMass,
        Self::ShellSpeed,
        Self::TankMass,
        Self::TankSpeed,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Strength => "SEAM STRENGTH",
            Self::Modulus => "STIFFNESS",
            Self::Density => "OBJECT DENSITY",
            Self::GrainSize => "GRAIN SIZE",
            Self::FractureWork => "FRACTURE WORK",
            Self::ShellMass => "SHELL MASS",
            Self::ShellSpeed => "SHELL SPEED",
            Self::TankMass => "TANK MASS",
            Self::TankSpeed => "TANK SPEED",
            Self::YieldStress => "YIELD STRESS",
            Self::PlasticFlow => "PLASTIC FLOW RATE",
            Self::Hardening => "HARDENING",
            Self::StrainLimit => "STRAIN LIMIT",
        }
    }
}

impl Experiment {
    pub fn sweep(&mut self, reference: usize, parameter: Sweep) {
        self.sweep_count(reference, parameter, 4);
    }
    pub fn sweep_count(&mut self, reference: usize, parameter: Sweep, count: usize) {
        let original = self.variants[reference].clone();
        let factors: &[f32] = if count == 6 {
            &[1.0 / 3.0, 2.0 / 3.0, 1.0, 4.0 / 3.0, 11.0 / 6.0, 2.5]
        } else {
            &[0.5, 1.0, 1.5, 2.0]
        };
        self.variants = factors
            .iter()
            .copied()
            .enumerate()
            .map(|(i, factor)| {
                let mut v = original.clone();
                v.name = if count == 6 {
                    format!("{} / {factor:.2}x", (b'A' + i as u8) as char)
                } else {
                    format!("{} / {factor:.1}x", (b'A' + i as u8) as char)
                };
                match parameter {
                    Sweep::StrainLimit => {
                        let p = v.object.components.plasticity.get_or_insert_default();
                        p.max_strain = (p.max_strain * factor).clamp(0.02, 3.0);
                    }
                    Sweep::YieldStress => {
                        let p = v.object.components.plasticity.get_or_insert_default();
                        p.yield_stress = (p.yield_stress * factor).clamp(100.0, 1e9);
                    }
                    Sweep::PlasticFlow => {
                        let p = v.object.components.plasticity.get_or_insert_default();
                        p.flow_rate = (p.flow_rate * factor).clamp(0.01, 20.0);
                    }
                    Sweep::Hardening => {
                        let p = v.object.components.plasticity.get_or_insert_default();
                        p.hardening = (p.hardening * factor).clamp(0.0, 100.0);
                    }
                    Sweep::Strength => {
                        v.object.tension = (v.object.tension * factor).clamp(1000.0, 1e9);
                        v.object.compression = (v.object.compression * factor).clamp(1000.0, 1e9);
                        v.object.shear = (v.object.shear * factor).clamp(1000.0, 1e9);
                    }
                    Sweep::Modulus => {
                        v.object.young = Some((v.object.young_modulus() * factor).clamp(1e5, 1e12))
                    }
                    Sweep::Density => {
                        v.object.density = (v.object.density * factor).clamp(100.0, 20_000.0)
                    }
                    Sweep::GrainSize => {
                        if let Some(l) = v.object.layout.clone() {
                            v.object.regenerate_grains(
                                l.mode,
                                l.size.map(|x| (x * factor).clamp(0.15, 4.0)),
                                l.seed,
                            );
                        } else {
                            v.object.refinement = (i as u8).min(4);
                        }
                    }
                    Sweep::FractureWork => {
                        v.object.toughness =
                            Some((v.object.fracture_energy() * factor).clamp(0.1, 100_000.0))
                    }
                    Sweep::ShellMass => {
                        let (lo, hi) = v.impact.ammunition.mass_range();
                        v.impact.shell_mass = (v.impact.shell_mass * factor).clamp(lo, hi)
                    }
                    Sweep::ShellSpeed => {
                        let (lo, hi) = v.impact.ammunition.speed_range();
                        v.impact.shell_speed = (v.impact.shell_speed * factor).clamp(lo, hi)
                    }
                    Sweep::TankMass => {
                        v.impact.tank_mass = (v.impact.tank_mass * factor).clamp(500.0, 30_000.0)
                    }
                    Sweep::TankSpeed => {
                        v.impact.tank_speed = (v.impact.tank_speed * factor).clamp(0.1, 12.0)
                    }
                }
                v
            })
            .collect();
        self.shared_impact &= self
            .variants
            .iter()
            .all(|v| v.impact == self.variants[0].impact);
        self.invalidate();
    }
}
