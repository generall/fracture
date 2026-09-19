//! Same-toolchain behavior fingerprints for refactors. Compare stdout before
//! and after a change; these are not cross-platform or cross-version promises.
use sim::{
    Ammunition, Command, LibraryPreset, Scene, Shot, Vec2, World,
    experiment::{Experiment, Runner},
};
use std::fmt::{Debug, Write};

struct Fingerprint(u64);
impl Write for Fingerprint {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        for byte in value.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        Ok(())
    }
}
impl Fingerprint {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn add(&mut self, value: impl Debug) {
        writeln!(self, "{value:?}").unwrap();
    }
}

fn run(name: &str, mut world: World, commands: impl Fn(u32) -> Vec<Command>) {
    let mut digest = Fingerprint::new();
    for tick in 0..480 {
        digest.add(world.step(&commands(tick)));
        digest.add(world.stats());
        digest.add(world.track_travel());
        digest.add(world.track_speeds());
        digest.add(world.tank_alive());
        digest.add(world.tank_fall_progress());
        digest.add(world.terrain_at(world.launcher_position()));
        digest.add(world.reload_remaining());
        digest.add(world.laser_beam());
        for body in world.bodies() {
            digest.add(body);
        }
        for bond in world.bonds() {
            digest.add(bond);
        }
        for shell in world.shells() {
            digest.add(shell);
        }
        for bridge in world.bridges() {
            digest.add((
                bridge.deck,
                bridge.controls,
                bridge.intact,
                bridge.fall_progress,
            ));
            digest.add(bridge.fragments);
        }
    }
    println!("{name}: {:016x}", digest.0);
}

fn main() {
    for scene in Scene::ALL
        .into_iter()
        .chain([Scene::ObjectYard, Scene::Terrain])
    {
        run(scene.title(), World::new(scene), |tick| {
            if tick % 60 == 0 {
                vec![Command::Fire(Shot::new(
                    Ammunition::HighExplosive,
                    Vec2::new(3.0, 0.0),
                ))]
            } else {
                vec![]
            }
        });
    }
    for preset in LibraryPreset::ALL {
        for ammo in Ammunition::ALL {
            run(
                &format!("{} / {}", preset.label(), ammo.label()),
                World::from_object(&preset.asset()).unwrap(),
                |tick| {
                    if ammo.automatic() || tick % 60 == 0 {
                        vec![Command::Fire(Shot::new(ammo, Vec2::new(3.0, 0.0)))]
                    } else {
                        vec![]
                    }
                },
            );
        }
    }
    for (name, position, speed) in [
        ("cross bridge", Vec2::new(-7.0, 0.0), 4.6),
        ("ram booth", Vec2::new(-7.0, 2.1), 4.6),
        ("bump booth", Vec2::new(-7.0, 2.1), 3.0),
        ("water", Vec2::new(-7.0, -4.0), 4.6),
        ("sand", Vec2::new(-9.5, -4.0), 4.6),
    ] {
        let mut w = World::new(Scene::Terrain);
        w.configure_test_tank(4500.0, position, speed).unwrap();
        run(name, w, |tick| {
            vec![Command::Tracks {
                left: if tick < 360 { 1.0 } else { 0.0 },
                right: if tick < 300 { 1.0 } else { 0.0 },
            }]
        });
    }
    let mut w = World::new(Scene::Terrain);
    w.configure_test_tank(4500.0, Vec2::ZERO, 0.0).unwrap();
    run("sink tank", w, |tick| {
        if tick % 60 == 0 {
            vec![Command::Fire(Shot::new(
                Ammunition::HighExplosive,
                Vec2::new(-3.0, 2.05),
            ))]
        } else {
            vec![]
        }
    });
    for e in [
        Experiment::barrel_comparison(),
        Experiment::crush_comparison(),
        Experiment::barrel_shot_comparison(),
        Experiment::barrel_free_shot_comparison(),
        Experiment::barrel_release_comparison(),
    ] {
        let mut runner = Runner::new(e.clone()).unwrap();
        while runner.advance().is_some() {}
        let mut digest = Fingerprint::new();
        digest.add(e.to_json().unwrap());
        for record in &runner.recordings {
            digest.add(&record.geometry);
            digest.add(&record.frames);
        }
        println!("{}: {:016x}", e.name, digest.0);
    }
}
