//! Reproducible standard-ammunition checks on accepted starters and a thin panel.
use sim::{Ammunition, Command, LibraryPreset, ObjectAsset, ObjectPreset, Shot, Vec2, World};

fn main() {
    let mut panel = ObjectAsset::workshop_preset(ObjectPreset::Plate);
    panel.name = "thin panel".into();
    panel.thickness = 0.02;
    for asset in [
        LibraryPreset::Wall.asset(),
        LibraryPreset::Rock.asset(),
        LibraryPreset::Barrel.asset(),
        LibraryPreset::ExplosiveBarrel.asset(),
        LibraryPreset::MetalFence.asset(),
        ObjectAsset::workshop_preset(ObjectPreset::Plate),
        panel,
    ] {
        for ammo in Ammunition::ALL {
            let mut w = World::from_object(&asset).unwrap();
            let commands = [Command::Fire(Shot::new(ammo, Vec2::new(3.0, 0.0)))];
            let mut impact = None;
            let mut cracks = Vec::new();
            for tick in 0..240 {
                let events = w.step(if tick == 0 || ammo.automatic() {
                    &commands
                } else {
                    &[]
                });
                for event in events {
                    match event {
                        sim::Event::Exploded { position, .. } if impact.is_none() => {
                            impact = Some(position)
                        }
                        sim::Event::Fractured { position, .. } => cracks.push(position),
                        _ => {}
                    }
                }
            }
            let s = w.stats();
            println!(
                "{} / {}: {} shots, {} blasts, {} cracks, {} chunks, {} subdivisions, {} cuts ({:.4}m²), strain {:.4}, KE {:.3}J, {} failed solves",
                asset.name,
                ammo.label(),
                s.shots,
                s.explosions,
                s.broken,
                s.chunks,
                s.refinements,
                s.ablated,
                s.ablated_area,
                s.plastic_strain,
                s.kinetic_energy,
                s.unconverged
            );
            if let Some(p) = impact {
                println!(
                    "  cracks within 0.65 m of impact: {}",
                    cracks.iter().filter(|q| q.distance(p) <= 0.65).count()
                );
            }
        }
    }
}
