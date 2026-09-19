//! Initial profile measurements; these are not acceptance targets for calibration.
use sim::{Command, LibraryPreset, Shot, Vec2, World};
fn main() {
    for preset in LibraryPreset::ALL {
        for tank in [false, true] {
            let asset = preset.asset();
            let mut w = World::from_object(&asset).unwrap();
            if tank {
                w.configure_test_tank(4500.0, Vec2::new(-1.0, 0.0), 4.6)
                    .unwrap();
            } else {
                w.step(&[Command::Fire(Shot {
                    ammunition: sim::Ammunition::HighExplosive,
                    target: Vec2::new(3.0, 0.0),
                    mass: 6.0,
                    speed: 220.0,
                })]);
            }
            for _ in 0..360 {
                w.step(if tank {
                    &[Command::Tracks {
                        left: 1.0,
                        right: 1.0,
                    }]
                } else {
                    &[]
                });
            }
            let s = w.stats();
            println!(
                "{} / {}: {} broken seams, {} chunks, {} refinements, {} explosions, {:.3} m2 cleared, {:.1}% strain, {:.1}J plastic work, {} failed solves",
                preset.label(),
                if tank { "tank" } else { "shell" },
                s.broken,
                s.chunks,
                s.refinements,
                s.explosions,
                s.cleared_area,
                s.plastic_strain * 100.0,
                s.plastic_work,
                s.unconverged
            );
        }
    }
}
