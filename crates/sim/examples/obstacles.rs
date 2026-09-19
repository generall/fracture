//! Compare obstacle tuning against the original barrier and prior wall profile.
//! --oblique alternates shell aim between the middle and either side.
use sim::{
    Command, LibraryPreset, Material, ObjectAsset, ObjectPreset, Scene, Shape, Shot, Vec2, World,
};
use std::collections::BTreeMap;

fn area(shape: Shape) -> f32 {
    match shape {
        Shape::Triangle {
            vertices: [a, b, c],
        } => (b - a).perp_dot(c - a).abs() * 0.5,
        _ => 0.0,
    }
}
fn report(label: &str, mut w: World, tank: bool) {
    let total: f32 = w.bodies().map(|b| area(b.shape)).sum();
    if tank {
        w.configure_test_tank(4500.0, Vec2::new(-1.0, 0.0), 4.6)
            .unwrap();
    }
    for hit in 0..3 {
        if !tank {
            w.step(&[Command::Fire(Shot {
                ammunition: sim::Ammunition::HighExplosive,
                target: Vec2::new(
                    3.0,
                    if std::env::args().any(|a| a == "--oblique") {
                        [0.0, 0.65, -0.65][hit]
                    } else {
                        0.0
                    },
                ),
                mass: 6.0,
                speed: 220.0,
            })]);
        }
        for _ in 0..120 {
            w.step(if tank {
                &[Command::Tracks {
                    left: 1.0,
                    right: 1.0,
                }]
            } else {
                &[]
            });
        }
        let mut chunks = BTreeMap::new();
        for b in w
            .bodies()
            .filter(|b| matches!(b.material, Material::Rock | Material::Masonry))
        {
            *chunks.entry(b.chunk).or_insert(0.0_f32) += area(b.shape);
        }
        let largest = chunks.values().copied().fold(0.0_f32, f32::max);
        let held: f32 = w.bodies().filter(|b| b.fixed).map(|b| area(b.shape)).sum();
        println!(
            "{label} / {} {}: core {:.3}/{total:.3} m² ({:.1}%), held {:.1}%, {} chunks, {} cracks, {} refinements, {} failed solves, {} fired / {} exploded",
            if tank { "tank second" } else { "shot" },
            hit + 1,
            largest,
            100.0 * largest / total,
            100.0 * held / total,
            chunks.len(),
            w.stats().broken,
            w.stats().refinements,
            w.stats().unconverged,
            w.stats().shots,
            w.stats().explosions
        );
    }
}
fn previous_wall() -> ObjectAsset {
    let mut a = ObjectAsset::workshop_preset(ObjectPreset::Wall);
    a.density = 2400.0;
    a.thickness = 0.55;
    a.friction = 1.3;
    a.cohesion = 18.0;
    a.toughness = Some(55.0);
    a.components.contact_resistance = 1.5;
    a.components.contact_refinement = false;
    a.regenerate_grains(sim::GrainMode::Bricks, [0.7, 0.6], 7);
    a
}

fn main() {
    for tank in [false, true] {
        report("original barrier", World::new(Scene::Masonry), tank);
        report(
            "previous wall",
            World::from_object(&previous_wall()).unwrap(),
            tank,
        );
        report(
            "updated wall",
            World::from_object(&LibraryPreset::Wall.asset()).unwrap(),
            tank,
        );
        report(
            "updated rock",
            World::from_object(&LibraryPreset::Rock.asset()).unwrap(),
            tank,
        );
    }
}
