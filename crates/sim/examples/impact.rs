//! Sweep impact energies and report actual geometric displacement.
use sim::{Command, Material, Scene, Shot, Vec2, World};

fn main() {
    for (mass, speed) in [
        (1.0, 80.0),
        (3.0, 150.0),
        (6.0, 220.0),
        (10.0, 300.0),
        (20.0, 400.0),
    ] {
        let mut world = World::new(Scene::Masonry);
        let initial: Vec<_> = world.bodies().map(|body| body.position).collect();
        world.step(&[Command::Fire(Shot {
            ammunition: sim::Ammunition::HighExplosive,
            target: Vec2::new(3.0, 0.0),
            mass,
            speed,
        })]);
        for _ in 0..240 {
            world.step(&[]);
        }
        let displacements: Vec<_> = world
            .bodies()
            .map(|body| (body, &initial[body.origin]))
            .filter(|(body, _)| body.material == Material::Masonry)
            .map(|(body, initial)| body.position.distance(*initial))
            .collect();
        println!(
            "{mass} kg @ {speed} m/s: {} broken, {} displaced >25 cm, max {:.2} m",
            world.stats().broken,
            displacements.iter().filter(|d| **d > 0.25).count(),
            displacements.iter().fold(0.0f32, |m, d| m.max(*d))
        );
    }
}
