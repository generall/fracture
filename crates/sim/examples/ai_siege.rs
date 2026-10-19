//! Focused regression probe using the real River Valley base geometry. Custom
//! deployment positions/vehicles isolate wall handling; these are not league results.
use sim::{
    Event, Vec2, VehicleKind,
    ai::{InputPlan, process::Worker},
    level::{Level, LevelItem, Team},
    multiplayer::{MultiplayerWorld, PlayerInput},
};
use std::{fs, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 7 {
        return Err(
            "ai_siege AGENT CONFIG blue|red tank|mortar|scout DIRECTION OUTPUT [--no-defences] [--from-spawn]"
                .into(),
        );
    }
    let team = if args[3] == "blue" {
        Team::Blue
    } else {
        Team::Red
    };
    let kind = match args[4].as_str() {
        "scout" => VehicleKind::Scout,
        "mortar" => VehicleKind::Mortar,
        _ => VehicleKind::Tank,
    };
    let direction: u32 = args[5].parse()?;
    let mut level = Level::from_json(include_str!("../../../levels/dota-level.json"))?;
    let goal = level
        .items
        .iter()
        .find_map(|i| match i {
            LevelItem::Flag { team: t, position } if *t != team => Some(Vec2::from(*position)),
            _ => None,
        })
        .unwrap();
    let offset = Vec2::from_angle(direction as f32 * std::f32::consts::FRAC_PI_2) * 12.0;
    let from_spawn = args.iter().any(|s| s == "--from-spawn");
    for item in &mut level.items {
        if let LevelItem::Spawn {
            team: t,
            position,
            angle,
        } = item
            && *t == team
            && !from_spawn
        {
            *position = (goal + offset).to_array();
            *angle = (-offset).to_angle();
        }
    }
    let defences = !args.iter().any(|s| s == "--no-defences");
    if !defences {
        level
            .items
            .retain(|i| !matches!(i, LevelItem::Turret { .. }));
    }
    let rocket = args.iter().any(|s| s == "--rocket");
    if rocket {
        let spawn = level
            .items
            .iter()
            .find_map(|i| match i {
                LevelItem::Spawn {
                    team: t, position, ..
                } if *t == team => Some(*position),
                _ => None,
            })
            .unwrap();
        level.items.push(LevelItem::Pickup {
            weapon: sim::SpecialWeapon::Rocket,
            position: spawn,
            respawn_time: 60.0,
            charges: 3,
            cooldown: 5.0,
        });
    }
    let mut game = MultiplayerWorld::new(&level, &[team, team.other()])?;
    game.require_vehicle_choices();
    let mut bot = Worker::new(&args[1], &args[2], game.for_player(0).unwrap().ai_map())?;
    let mut input = [PlayerInput::default(); 2];
    input[0].deploy = Some(kind);
    input[1].deploy = Some(VehicleKind::Tank);
    game.step(&input);
    let mut trace = std::io::BufWriter::new(fs::File::create(&args[6])?);
    let mut blocked = 0;
    let mut plan = InputPlan::default();
    let mut closest = f32::MAX;
    let mut captured = false;
    let mut elapsed = 0;
    for tick in 0..if from_spawn { 28800 } else { 7200 } {
        elapsed = tick;
        if tick % 12 == 0 {
            let o = game.for_player(0).unwrap().ai_observation();
            closest = closest.min(o.position.distance(goal));
            let decision = bot.decide(&o)?;
            plan = decision.plan();
            input[0] = decision.input;
            // Hold the probe vehicle constant after deaths; normal league
            // rounds retain uniform random deployment choices.
            input[0].deploy = Some(kind);
            serde_json::to_writer(
                &mut trace,
                &serde_json::json!({"tick":tick,
                "position":o.position,"health":o.health,"alive":o.alive,"spread":o.aim_spread,
                "special":o.special,
                "input":input[0],"telemetry":decision.telemetry,"obstacles":o.obstacles,
                "reload":o.reload,"turret":o.turret_angle}),
            )?;
            writeln!(trace)?;
        }
        input[0] = plan.input_at((tick % 12) as usize);
        input[0].deploy = Some(kind);
        let events = game.step(&input);
        blocked += events
            .iter()
            .filter(|e| matches!(e, Event::MuzzleBlocked))
            .count();
        if game.world().winner() == Some(team) {
            captured = true;
            break;
        }
    }
    let result = serde_json::json!({"probe":true,"team":team,"vehicle":kind,"direction":direction,
        "defences":defences,"from_spawn":from_spawn,"rocket":rocket,"captured":captured,"seconds":elapsed as f32/120.0,"closest":closest,
        "shots":game.shots(0),"muzzle_blocked":blocked,"version":bot.config.name});
    fs::write(
        format!("{}.summary.json", args[6]),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{result}");
    Ok(())
}
