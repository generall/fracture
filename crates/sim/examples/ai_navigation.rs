//! Real-map driving probes with combat disabled. These are not league matches.
use sim::{
    Vec2, VehicleKind,
    ai::{InputPlan, process::Worker},
    level::{Level, LevelItem, Team},
    multiplayer::{MultiplayerWorld, PlayerInput},
};
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err(
            "ai_navigation AGENT CONFIG OUTPUT [scout|tank|mortar] [--raiding-navigation]".into(),
        );
    }
    let output = PathBuf::from(&args[3]);
    fs::create_dir_all(&output)?;
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let raiding_navigation = args.iter().any(|v| v == "--raiding-navigation");
    let kind_filter = args.iter().skip(4).find(|v| !v.starts_with("--"));
    // Isolate the same navigation implementation from enemy combat/assault.
    for key in [
        "siege_tactics",
        "engagement_spacing",
        "team_roles",
        "clear_fire_lane",
        "tactical_routes",
        "raiding",
        "wide_routes",
        "supply_runs",
    ] {
        if config.get(key).is_some() {
            config[key] = (raiding_navigation && key == "raiding").into();
        }
    }
    let config_path = output.join("navigation-config.json");
    fs::write(&config_path, serde_json::to_vec_pretty(&config)?)?;
    let cases = [
        ("river-out", [15.0, -23.0], [-15.0, 25.0], 2.2),
        ("river-back", [-15.0, 25.0], [15.0, -23.0], -0.9),
        ("rocks-out", [32.0, -55.0], [10.0, -21.0], 2.1),
        ("rocks-back", [10.0, -21.0], [32.0, -55.0], -1.0),
        (
            "spawn-out",
            [43.3536, -73.3322],
            [15.0, -23.0],
            std::f32::consts::FRAC_PI_2,
        ),
        ("spawn-back", [15.0, -23.0], [43.3536, -68.0], -1.0),
        ("garage-entry", [15.0, -23.0], [43.3536, -73.3322], -1.0),
    ];
    let mut results = vec![];
    for kind in VehicleKind::ALL {
        let label = format!("{kind:?}").to_lowercase();
        if kind_filter.is_some_and(|v| *v != label) {
            continue;
        }
        for (name, start, goal, angle) in cases {
            if name == "garage-entry" && kind != VehicleKind::Scout {
                continue;
            }
            let mut level = Level::from_json(include_str!("../../../levels/dota-level.json"))?;
            level
                .items
                .retain(|i| !matches!(i, LevelItem::Turret { .. } | LevelItem::Pickup { .. }));
            for item in &mut level.items {
                if let LevelItem::Spawn {
                    team: Team::Blue,
                    position,
                    angle: facing,
                } = item
                {
                    *position = start;
                    *facing = angle;
                }
            }
            let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red])?;
            game.require_vehicle_choices();
            let mut map = game.for_player(0).unwrap().ai_map();
            map.flags = vec![(Team::Red, goal.into())];
            let mut worker = Worker::new(&args[1], config_path.to_str().unwrap(), map)?;
            let mut input = [PlayerInput::default(); 2];
            input[0].deploy = Some(kind);
            game.step(&input);
            let mut trace = std::io::BufWriter::new(fs::File::create(
                output.join(format!("{label}-{name}.jsonl")),
            )?);
            let mut reached = false;
            let mut plan = InputPlan::default();
            let mut closest = f32::INFINITY;
            let mut minimum_health = 100.0_f32;
            let mut elapsed = 0;
            let mut max_stall = 0;
            let mut progress_tick = 0;
            let mut progress = Vec2::from(start);
            for tick in 0..10800 {
                elapsed = tick;
                if tick % 12 == 0 {
                    let mut o = game.for_player(0).unwrap().ai_observation();
                    if o.alive {
                        minimum_health = minimum_health.min(o.health);
                    }
                    closest = closest.min(o.position.distance(goal.into()));
                    if o.alive && closest < 2.0 {
                        reached = true;
                        break;
                    }
                    if o.position.distance(progress) > 0.75 {
                        progress = o.position;
                        progress_tick = tick;
                    }
                    max_stall = max_stall.max(tick - progress_tick);
                    o.targets.clear();
                    let decision = worker.decide(&o)?;
                    plan = decision.plan();
                    input[0] = decision.input;
                    input[0].deploy = Some(kind);
                    input[0].fire = false;
                    input[0].special = false;
                    serde_json::to_writer(
                        &mut trace,
                        &serde_json::json!({"tick":tick,"position":o.position,"hull":o.hull_angle,"health":o.health,"input":input[0],"telemetry":decision.telemetry}),
                    )?;
                    writeln!(trace)?;
                }
                input[0] = plan.input_at((tick % 12) as usize);
                input[0].deploy = Some(kind);
                input[0].fire = false;
                input[0].special = false;
                game.step(&input);
            }
            let result = serde_json::json!({"case":name,"vehicle":kind,"raiding_navigation":raiding_navigation,"reached":reached,"seconds":elapsed as f32/120.0,"closest":closest,"minimum_health":minimum_health,"max_stall_seconds":max_stall as f32/120.0});
            println!("{result}");
            results.push(result);
        }
    }
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    if results.iter().any(|r| r["reached"] != true) {
        std::process::exit(1);
    }
    Ok(())
}
