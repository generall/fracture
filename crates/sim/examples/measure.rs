//! Reproducible native smoke scenario and timing report. Not a browser benchmark.
use sim::{Command, Scene, Shot, Vec2, World};
use std::time::Instant;

fn main() {
    for scene in Scene::ALL {
        let mut world = World::new(scene);
        let initial: Vec<_> = world.bodies().map(|body| body.position).collect();
        let mut times = Vec::new();
        for tick in 0..1_200 {
            let commands = if tick % 180 == 0 {
                vec![Command::Fire(Shot {
                    ammunition: sim::Ammunition::HighExplosive,
                    target: Vec2::new(3.0, ((tick / 180) % 5) as f32 - 2.0),
                    mass: 6.0,
                    speed: 220.0,
                })]
            } else {
                vec![]
            };
            let start = Instant::now();
            world.step(&commands);
            times.push(start.elapsed().as_secs_f64() * 1_000.0);
            if tick == 120 {
                let displaced = world
                    .bodies()
                    .map(|body| (body, &initial[body.origin]))
                    .filter(|(body, initial)| {
                        matches!(
                            body.material,
                            sim::Material::Masonry | sim::Material::Panel | sim::Material::Rock
                        ) && body.position.distance(**initial) > 0.25
                    })
                    .count();
                println!(
                    "After one shot: {} broken connections, {displaced} pieces displaced > 25 cm",
                    world.stats().broken
                );
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{scene:?}: {:?}; p95 {:.3} ms, max {:.3} ms",
            world.stats(),
            times[times.len() * 95 / 100],
            times.last().unwrap()
        );
    }
}
