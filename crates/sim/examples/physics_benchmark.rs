//! Headless fixed-input workloads for stage profiling and thread-count comparisons.
//! Build with --features physics-profile; add parallel to test Rapier worker pools.
use sim::{
    Ammunition, Command, Material, Scene, Shot, Vec2, World,
    level::{Level, Team},
};
use std::{fmt::Write, time::Instant};

struct Fingerprint(u64);
impl Write for Fingerprint {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        for byte in value.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        Ok(())
    }
}
fn fingerprint(world: &World, digest: &mut Fingerprint) {
    write!(
        digest,
        "{:?}{:?}{:?}",
        world.stats(),
        world.track_travel(),
        world.player_aim()
    )
    .unwrap();
    for body in world.bodies() {
        write!(digest, "{body:?}").unwrap();
    }
    for bond in world.bonds() {
        write!(digest, "{bond:?}").unwrap();
    }
    for shell in world.shells() {
        write!(digest, "{shell:?}").unwrap();
    }
}
fn world(name: &str) -> World {
    match name {
        "masonry" => World::new(Scene::Masonry),
        "barrel" => World::new(Scene::BarrelRubble),
        "river" | "classic" => {
            let json = if name == "river" {
                include_str!("../../../levels/dota-level.json")
            } else {
                include_str!("../../../levels/classic-tanks-2.json")
            };
            let mut level = Level::from_json(json).unwrap();
            // Keep the workload alive for its full duration; no AI flag capture.
            level.enemy_activity = sim::level::EnemyActivity::Static;
            World::from_level(&level, Team::Blue).unwrap()
        }
        _ => panic!("unknown scenario {name}; use river, classic, masonry, barrel or all"),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let option = |flag: &str| {
        args.iter().position(|s| s == flag).map(|i| {
            args.get(i + 1)
                .unwrap_or_else(|| panic!("missing {flag} value"))
                .as_str()
        })
    };
    let ticks: usize = option("--ticks").unwrap_or("1200").parse().unwrap();
    let threads: usize = option("--threads").unwrap_or("1").parse().unwrap();
    let selected = option("--scenario").unwrap_or("all");
    let verify = args.iter().any(|a| a == "--verify");
    let global_pool = args.iter().any(|a| a == "--global-pool");
    #[cfg(not(feature = "parallel"))]
    assert!(
        !global_pool,
        "build with --features parallel to use a global pool"
    );
    if global_pool {
        assert_eq!(
            std::env::var("RAYON_NUM_THREADS")
                .unwrap()
                .parse::<usize>()
                .unwrap(),
            threads,
            "set RAYON_NUM_THREADS to match --threads for the global pool"
        );
    }
    assert!(ticks > 0 && threads > 0);
    #[cfg(not(feature = "parallel"))]
    assert_eq!(
        threads, 1,
        "build with --features parallel to use worker threads"
    );
    for name in ["river", "classic", "masonry", "barrel"] {
        if selected != "all" && selected != name {
            continue;
        }
        let mut world = world(name);
        #[cfg(feature = "parallel")]
        if !global_pool {
            world.configure_physics_threads(threads).unwrap();
        }
        // Select nearby original structures once, so all variants get identical aims.
        let origin = world.launcher_position();
        let mut points: Vec<_> = world
            .bodies()
            .filter(|b| !matches!(b.material, Material::Boundary | Material::Launcher))
            .map(|b| b.position)
            .filter(|p| p.distance(origin) > 3.0)
            .collect();
        points.sort_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        });
        let mut targets: Vec<Vec2> = vec![];
        for p in points {
            if targets.iter().all(|q| q.distance(p) > 3.0) {
                targets.push(p);
            }
            if targets.len() == 4 {
                break;
            }
        }
        assert!(!targets.is_empty());
        world.take_physics_profile(); // Exclude map construction and initial topology.
        let mut samples = Vec::with_capacity(ticks);
        let mut stages = serde_json::Map::new();
        let mut worst = serde_json::Value::Null;
        let mut worst_ms = 0.0;
        let mut digest = Fingerprint(0xcbf29ce484222325);
        for tick in 0..ticks {
            let commands = if name == "barrel" {
                vec![Command::Tracks {
                    left: 1.0,
                    right: 1.0,
                }]
            } else {
                let aim = targets[(tick / 240) % targets.len()];
                let mut commands = vec![Command::Fire(Shot::new(Ammunition::HighExplosive, aim))];
                if world.scene() == Scene::Level {
                    commands.push(Command::Aim(aim));
                }
                commands
            };
            let start = Instant::now();
            let events = world.step(&commands);
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            let profile = serde_json::to_value(world.take_physics_profile()).unwrap();
            for (key, value) in profile.as_object().unwrap() {
                let entry = stages.entry(key.clone()).or_insert(serde_json::json!(0.0));
                *entry = serde_json::json!(entry.as_f64().unwrap() + value.as_f64().unwrap());
            }
            if ms > worst_ms {
                worst_ms = ms;
                worst = serde_json::json!({"tick":tick, "ms":ms, "stages_ms":profile});
            }
            samples.push(ms);
            if verify {
                write!(digest, "{events:?}").unwrap();
                fingerprint(&world, &mut digest);
            }
        }
        if !verify {
            fingerprint(&world, &mut digest);
        }
        assert_eq!(world.stats().ticks as usize, ticks, "benchmark ended early");
        let total: f64 = samples.iter().sum();
        let missed = samples.iter().filter(|&&ms| ms > 1000.0 / 120.0).count();
        samples.sort_by(f64::total_cmp);
        let percentile = |p: f64| samples[((samples.len() - 1) as f64 * p) as usize];
        println!(
            "{}",
            serde_json::json!({
                "scenario":name,"parallel":cfg!(feature="parallel"),"global_pool":global_pool,"threads":threads,"ticks":ticks,
                "verify_every_tick":verify,"fingerprint":format!("{:016x}",digest.0),
                "total_ms":total,"mean_ms":total/ticks as f64,"p50_ms":percentile(0.5),
                "p95_ms":percentile(0.95),"p99_ms":percentile(0.99),"max_ms":worst_ms,
                "over_budget_ticks":missed,"stages_total_ms":stages,"worst_tick":worst,"stats":world.stats()
            })
        );
    }
    assert!(
        ["all", "river", "classic", "masonry", "barrel"].contains(&selected),
        "unknown scenario {selected}"
    );
}
