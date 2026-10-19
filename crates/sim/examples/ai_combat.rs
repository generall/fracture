//! Controlled real-physics combat probes, not competitive league results.
use sim::{
    VehicleKind,
    ai::{InputPlan, process::Worker},
    level::{Level, LevelItem, Team},
    multiplayer::{MultiplayerWorld, PlayerInput},
};
use std::{fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err("ai_combat AGENT CONFIG OUTPUT".into());
    }
    let output = PathBuf::from(&args[3]);
    fs::create_dir_all(&output)?;
    let mut results = vec![];
    for kind in VehicleKind::ALL {
        for moving in [false, true] {
            let mut level = Level {
                size: [100.0, 80.0],
                ..Default::default()
            };
            for item in &mut level.items {
                match item {
                    LevelItem::Spawn {
                        team,
                        position,
                        angle,
                    } => {
                        *position = if *team == Team::Blue {
                            [-10.0, -20.0]
                        } else {
                            [10.0, -20.0]
                        };
                        *angle = if *team == Team::Blue {
                            0.0
                        } else {
                            std::f32::consts::FRAC_PI_2
                        };
                    }
                    LevelItem::Flag { position, .. } => *position = [40.0, 30.0],
                    _ => {}
                }
            }
            let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red])?;
            game.require_vehicle_choices();
            let enemy = if moving {
                VehicleKind::Scout
            } else {
                VehicleKind::Tank
            };
            let mut inputs = [
                PlayerInput {
                    deploy: Some(kind),
                    ..Default::default()
                },
                PlayerInput {
                    deploy: Some(enemy),
                    ..Default::default()
                },
            ];
            for _ in 0..3 {
                game.step(&inputs);
            }
            let mut worker = Worker::new(&args[1], &args[2], game.for_player(0).unwrap().ai_map())?;
            let label = format!(
                "{:?}-{}",
                kind,
                if moving { "crossing" } else { "stationary" }
            )
            .to_lowercase();
            let mut trace =
                std::io::BufWriter::new(fs::File::create(output.join(format!("{label}.jsonl")))?);
            let mut first_shot = None;
            let mut plan = InputPlan::default();
            let mut defeated = false;
            let mut elapsed = 0;
            let mut minimum_enemy = 100.0_f32;
            if moving {
                inputs[1].tracks = [0.35, 0.35];
            }
            for tick in 0..1800 {
                elapsed = tick;
                if tick % 12 == 0 {
                    let o = game.for_player(0).unwrap().ai_observation();
                    let result = worker.decide(&o)?;
                    plan = result.plan();
                    inputs[0] = result.input;
                    serde_json::to_writer(
                        &mut trace,
                        &serde_json::json!({"tick":tick,"position":o.position,"hull":o.hull_angle,"turret":o.turret_angle,"spread":o.aim_spread,"targets":o.targets,"input":inputs[0],"telemetry":result.telemetry}),
                    )?;
                    writeln!(trace)?;
                }
                inputs[0] = plan.input_at((tick % 12) as usize);
                game.step(&inputs);
                if game.shots(0) > 0 && first_shot.is_none() {
                    first_shot = Some(tick as f32 / 120.0);
                }
                let enemy = game.for_player(1).unwrap().ai_observation();
                minimum_enemy = minimum_enemy.min(enemy.health);
                if !enemy.alive {
                    defeated = true;
                    break;
                }
            }
            let result = serde_json::json!({"probe":true,"case":label,"vehicle":kind,"moving_target":moving,"defeated":defeated,"seconds":elapsed as f32/120.0,"first_shot_seconds":first_shot,"shots":game.shots(0),"minimum_enemy_health":minimum_enemy,"version":worker.config.name});
            println!("{result}");
            results.push(result);
        }
    }
    fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    Ok(())
}
