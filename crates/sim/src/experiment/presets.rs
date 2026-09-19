//! Prepared material studies and their accepted historical favorites.
use super::*;

impl Experiment {
    /// A controlled 2 x 5 barrel study: hardening versus plastic yield
    /// resistance. Geometry, mass, impact and backstop are identical in all cases.
    pub fn barrel_comparison() -> Self {
        let mut e = Self::new(crate::LibraryPreset::Barrel.asset());
        e.name = "Barrels / permanent dents / 10 cases".into();
        e.shared_impact = true;
        e.duration_ticks = 480;
        e.variants.clear();
        for hardening in [0.0, 4.0] {
            for yield_stress in [500.0, 1000.0, 2000.0, 4000.0, 8000.0] {
                let mut object = crate::LibraryPreset::Barrel.asset();
                object.soft_bounce = None; // Preserve the historical comparison.
                object.components.plasticity = Some(crate::Plasticity {
                    yield_stress,
                    hardening,
                    ..crate::Plasticity::default()
                });
                let number = e.variants.len() + 1;
                object.name = format!("Barrel {number:02}");
                e.variants.push(Variant {
                    name: format!("{number:02} / H {hardening:.0}"),
                    object,
                    impact: Impact {
                        kind: ImpactKind::Tank,
                        tank_speed: 0.8,
                        backstop: true,
                        ..Impact::default()
                    },
                });
            }
        }
        e
    }
    /// Six deformation limits under identical loading, using the accepted
    /// rim-only deformation model. All other object properties are identical.
    pub fn crush_comparison() -> Self {
        let mut object = crate::LibraryPreset::Barrel.asset();
        object.name = "Barrel / soft frame".into();
        object.soft_bounce = None;
        let mut e = Self::new(object.clone());
        e.name = "Barrels / deformation limit / 6 cases".into();
        e.shared_impact = true;
        e.duration_ticks = 960;
        e.variants.clear();
        for limit in [0.4, 0.8, 1.2, 1.6, 2.2, 3.0] {
            let mut object = object.clone();
            object.components.plasticity.as_mut().unwrap().max_strain = limit;
            e.variants.push(Variant {
                name: format!("{} / LIMIT {limit:.1}", e.variants.len() + 1),
                object,
                impact: Impact {
                    kind: ImpactKind::Tank,
                    tank_speed: 0.8,
                    backstop: true,
                    ..Impact::default()
                },
            });
        }
        e.favorite = Some(5); // The user's selected barrel profile.
        e
    }
    /// Rows vary collision restitution, columns vary surface/floor friction.
    /// All cases retain the selected barrel's deformation profile.
    pub fn barrel_shot_comparison() -> Self {
        let mut e = Self::new(crate::LibraryPreset::Barrel.asset());
        e.name = "Barrels / shot response / 6 cases".into();
        e.shared_impact = true;
        e.duration_ticks = 360;
        e.variants.clear();
        for restitution in [0.04, 0.7] {
            for friction in [0.02, 0.09, 0.4] {
                let mut object = crate::LibraryPreset::Barrel.asset();
                object.friction = friction;
                object.restitution = restitution;
                object.soft_bounce = None;
                e.variants.push(Variant {
                    name: format!(
                        "{} / {} BOUNCE",
                        e.variants.len() + 1,
                        if restitution < 0.1 { "LOW" } else { "HIGH" }
                    ),
                    object,
                    impact: Impact {
                        backstop: true,
                        ..Impact::default()
                    },
                });
            }
        }
        e.favorite = Some(5); // Preferred with the close backstop.
        e
    }
    /// The same six materials with no close backstop. Outer arena walls remain.
    pub fn barrel_free_shot_comparison() -> Self {
        let mut e = Self::barrel_shot_comparison();
        e.name = "Barrels / free shot / 6 cases".into();
        e.favorite = Some(1); // Preferred without the close backstop.
        for v in &mut e.variants {
            v.impact.backstop = false;
        }
        e
    }
    pub fn barrel_release_comparison() -> Self {
        let mut e = Self::barrel_shot_comparison();
        e.name = "Barrels / wall release / 6 cases".into();
        e.favorite = Some(1); // Selected wall-release case 2.
        for (i, (v, soft)) in e
            .variants
            .iter_mut()
            .zip([0.6, 0.62, 0.64, 0.66, 0.68, 0.7])
            .enumerate()
        {
            v.object = crate::LibraryPreset::Barrel.asset();
            v.object.soft_bounce.as_mut().unwrap().restitution = soft;
            v.name = format!("{} / SOFT {soft:.2}", i + 1);
        }
        e
    }
    pub fn new(object: ObjectAsset) -> Self {
        let sweep = if object.components.plasticity.is_some() {
            Sweep::YieldStress
        } else {
            Sweep::Strength
        };
        let variant = Variant {
            name: "Reference".into(),
            object,
            impact: Impact::default(),
        };
        let mut result = Self {
            version: 1,
            engine: ENGINE_REVISION.into(),
            name: "Material trial".into(),
            variants: vec![variant],
            shared_impact: false,
            duration_ticks: 720,
            cursor_tick: 0,
            favorite: None,
            review: vec![],
        };
        result.sweep(0, sweep);
        result
    }
}
