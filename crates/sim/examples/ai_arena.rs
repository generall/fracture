//! Silent, fixed-tick, isolated-process AI matches. See docs/AI_ARENA.md.
use serde::{Deserialize, Serialize};
use sim::ai::{ARENA_PLAYERS, ARENA_TEAMS, player_seed};
use sim::{
    Event, PhysicsProfile,
    ai::{
        Config, Controller, InputPlan, Map, Observation, Telemetry,
        process::{Decision, Worker as Bot},
    },
    level::{Level, Team},
    multiplayer::MultiplayerWorld,
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufRead, BufWriter, Write},
    path::PathBuf,
    time::Instant,
};

#[derive(Serialize, Deserialize)]
struct Init {
    config: Config,
    map: Map,
}
fn worker() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let init: Init = serde_json::from_str(&lines.next().ok_or("missing initialization")??)?;
    init.config.validate()?;
    let mut bot = Controller::new(init.config, init.map);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for line in lines {
        let line = line?;
        if let Some(json) = line.strip_prefix("RESET ") {
            let init: Init = serde_json::from_str(json)?;
            bot = Controller::new(init.config, init.map);
            continue;
        }
        let o: Observation = serde_json::from_str(&line)?;
        let start = Instant::now();
        let input = bot.decide(&o);
        let plan = bot.plan_input(input);
        let cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
        serde_json::to_writer(
            &mut out,
            &Decision {
                input,
                aim_path: plan.aim_path,
                telemetry: bot.telemetry.clone(),
                cpu_ms,
            },
        )?;
        writeln!(out)?;
        out.flush()?;
    }
    Ok(())
}
#[derive(Default, Serialize)]
struct Side {
    captures: u32,
    kills: u32,
    damage: f32,
    closest_flag: f32,
    distance: f32,
    stuck_seconds: f32,
    recoveries: u32,
    shots: u64,
    stuck_locations: Vec<[f32; 3]>,
}
#[derive(Default, Serialize)]
struct Timing {
    mean: f64,
    p95: f64,
    p99: f64,
    max: f64,
    over_8ms: usize,
}
fn timing(samples: &[f64]) -> Timing {
    if samples.is_empty() {
        return Timing::default();
    }
    let mut s = samples.to_vec();
    s.sort_by(f64::total_cmp);
    Timing {
        mean: s.iter().sum::<f64>() / s.len() as f64,
        p95: s[s.len() * 95 / 100],
        p99: s[s.len() * 99 / 100],
        max: *s.last().unwrap(),
        over_8ms: s.iter().filter(|&&v| v > 1000.0 / 120.0).count(),
    }
}
fn json_line(out: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    writeln!(out)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().filter(|s| s != "--summary-only").collect();
    let record = !std::env::args().any(|s| s == "--summary-only");
    if args.get(1).map(String::as_str) == Some("--worker") {
        return worker();
    }
    if args.get(1).map(String::as_str) == Some("--observe-benchmark") {
        let level = Level::from_json(&fs::read_to_string(args.get(2).ok_or("missing level")?)?)?;
        let mut game = MultiplayerWorld::new(&level, &ARENA_TEAMS)?;
        let mut fresh = vec![];
        let mut shared = vec![];
        for _ in 0..100 {
            let start = Instant::now();
            let a: Vec<_> = (0..ARENA_PLAYERS)
                .map(|i| game.for_player(i).unwrap().ai_observation())
                .collect();
            fresh.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            let observed_geometry = game.world().ai_geometry();
            let b: Vec<_> = (0..ARENA_PLAYERS)
                .map(|i| {
                    game.for_player(i)
                        .unwrap()
                        .ai_observation_from(&observed_geometry)
                })
                .collect();
            shared.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(serde_json::to_value(a)?, serde_json::to_value(b)?);
        }
        let result = serde_json::json!({"identical_observations":400,"fresh_batch_ms":timing(&fresh),"shared_batch_ms":timing(&shared)});
        fs::write(
            args.get(3).ok_or("missing output")?,
            serde_json::to_vec_pretty(&result)?,
        )?;
        println!("{result}");
        return Ok(());
    }
    if !(7..=9).contains(&args.len()) {
        return Err(
            "usage: ai_arena LEVEL BLUE_BINARY BLUE_CONFIG RED_BINARY RED_CONFIG OUTPUT_DIR [SECONDS] [SEED]".into(),
        );
    }
    let seconds: u32 = args
        .get(7)
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(sim::ai::ROUND_SECONDS);
    if !(1..=3600).contains(&seconds) {
        return Err("Round duration must be 1–3600 seconds".into());
    }
    let ticks = seconds * 120;
    let seed: u64 = args.get(8).map(|v| v.parse()).transpose()?.unwrap_or(42);
    let level = Level::from_json(&fs::read_to_string(&args[1])?)?;
    let out = PathBuf::from(&args[6]);
    fs::create_dir_all(&out)?;
    let mut game = MultiplayerWorld::new(&level, &ARENA_TEAMS)?;
    game.configure_airdrops(true, 42);
    game.require_vehicle_choices();
    let mut make_map = |side| {
        let mut map = game.for_player(side).unwrap().ai_map();
        map.seed = player_seed(seed, side);
        map.team_slot = side / 2;
        map
    };
    let mut bots = [
        Bot::new(&args[2], &args[3], make_map(0))?,
        Bot::new(&args[4], &args[5], make_map(1))?,
        Bot::new(&args[2], &args[3], make_map(2))?,
        Bot::new(&args[4], &args[5], make_map(3))?,
    ];
    let output = |name| -> io::Result<Box<dyn Write>> {
        Ok(if record {
            Box::new(BufWriter::new(File::create(out.join(name))?))
        } else {
            Box::new(io::sink())
        })
    };
    let mut decisions = output("decisions.jsonl")?;
    let mut frames = output("frames.jsonl")?;
    let mut evaluation = BufWriter::new(File::create(out.join("evaluation.jsonl"))?);
    let mut geometry = Vec::new();
    let mut shapes = BTreeMap::new();
    let mut poses = BTreeMap::new();
    let mut sides: [Side; 2] = std::array::from_fn(|_| Side {
        closest_flag: f32::MAX,
        ..Default::default()
    });
    let mut plans: [InputPlan; ARENA_PLAYERS] = std::array::from_fn(|_| InputPlan::default());
    let mut telemetry: [Telemetry; ARENA_PLAYERS] = std::array::from_fn(|_| Telemetry::default());
    let mut step_ms = vec![];
    let mut observation_ms = vec![];
    let mut bot_ms = vec![];
    let mut ipc_ms = vec![];
    let mut profile = PhysicsProfile::default();
    let mut previous = [None; ARENA_PLAYERS];
    let mut health = [100.0; ARENA_PLAYERS];
    let mut stuck_anchor = [None; ARENA_PLAYERS];
    let mut stuck_since = [0u32; ARENA_PLAYERS];
    let mut leg = 0;
    let mut events_log = vec![];
    let mut slow_ticks = vec![];
    let mut setup_profile = game.take_physics_profile();
    for tick in 0..=ticks {
        if tick % 12 == 0 {
            let geometry_started = Instant::now();
            let observed_geometry = game.world().ai_geometry();
            let geometry_per_player =
                geometry_started.elapsed().as_secs_f64() * 1000.0 / ARENA_PLAYERS as f64;
            let mut views = vec![];
            let mut team_vision: [Vec<sim::Vision>; 2] = std::array::from_fn(|_| vec![]);
            let mut team_targets: [Vec<sim::ai::Target>; 2] = std::array::from_fn(|_| vec![]);
            for player in 0..ARENA_PLAYERS {
                let side = player % 2;
                let start = Instant::now();
                let o = game
                    .for_player(player)
                    .unwrap()
                    .ai_observation_from(&observed_geometry);
                observation_ms.push(start.elapsed().as_secs_f64() * 1000.0 + geometry_per_player);
                let start = Instant::now();
                let decision = bots[player].decide(&o)?;
                ipc_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                bot_ms.push(decision.cpu_ms);
                plans[player] = decision.plan();
                telemetry[player] = decision.telemetry.clone();
                if record {
                    team_vision[side].extend(o.regions.iter().cloned());
                    team_targets[side].extend(o.targets.iter().copied());
                    json_line(
                        &mut decisions,
                        &serde_json::json!({"tick":tick,"side":side,"player":player,"leg":leg,"observation":o,"decision":decision}),
                    )?;
                }
                json_line(
                    &mut evaluation,
                    &serde_json::json!({"tick":tick,"player":player,"leg":leg,
                    "position":o.position,"vehicle":o.vehicle,"alive":o.alive,"health":o.health,
                    "hull":o.hull_angle,"turret":o.turret_angle,"spread":o.aim_spread,"reload":o.reload,
                    "input":decision.input,"aim_path":decision.aim_path,"targets":o.targets,"allies":o.allied_vehicles,"telemetry":decision.telemetry}),
                )?;
                if let Some(p) = previous[player] {
                    sides[side].distance += o.position.distance(p);
                }
                previous[player] = Some(o.position);
                if o.alive {
                    sides[1 - side].damage += (health[player] - o.health).max(0.0);
                }
                health[player] = o.health;
                let goal = game
                    .for_player(player)
                    .unwrap()
                    .ai_map()
                    .flags
                    .into_iter()
                    .find(|(team, _)| *team == if side == 0 { Team::Red } else { Team::Blue })
                    .unwrap()
                    .1;
                sides[side].closest_flag = sides[side].closest_flag.min(o.position.distance(goal));
                if !o.alive
                    || !telemetry[player].moving
                    || stuck_anchor[player].is_none_or(|p| o.position.distance(p) > 0.75)
                {
                    stuck_anchor[player] = Some(o.position);
                    stuck_since[player] = tick;
                } else if tick - stuck_since[player] >= 360 {
                    sides[side].stuck_seconds += 0.1;
                    if (tick - stuck_since[player]) % 120 == 0 {
                        sides[side].stuck_locations.push([
                            tick as f32 / 120.0,
                            o.position.x,
                            o.position.y,
                        ]);
                    }
                }
                sides[side].recoveries = sides[side].recoveries.max(telemetry[player].recoveries);
                if record {
                    views.push(serde_json::json!({"team":side,"player":player,"position":o.position,"hull":o.hull_angle,"turret":o.turret_angle,
                    "alive":o.alive,"health":o.health,"vehicle":o.vehicle,"special":o.special,"regions":if player<2 {o.regions.as_slice()}else{&[]},"targets":if player<2 {o.targets.as_slice()}else{&[]},"telemetry":telemetry[player]}));
                }
            }
            if record {
                for side in 0..2 {
                    views[side]["regions"] = serde_json::to_value(&team_vision[side])?;
                    team_targets[side].sort_by(|a, b| {
                        a.position
                            .x
                            .total_cmp(&b.position.x)
                            .then(a.position.y.total_cmp(&b.position.y))
                    });
                    team_targets[side].dedup_by(|a, b| {
                        a.position.distance_squared(b.position) < 0.001 && a.mounted == b.mounted
                    });
                    views[side]["targets"] = serde_json::to_value(&team_targets[side])?;
                }
                let mut changes = vec![];
                let mut current = std::collections::BTreeSet::new();
                for body in game.world().bodies() {
                    current.insert(body.id);
                    let key = (body.id, body.shape_revision);
                    let shape=*shapes.entry(key).or_insert_with(||{
                    let i=geometry.len();geometry.push(serde_json::json!({"shape":body.shape,"material":body.material,"color":body.color}));i
                });
                    let pose = (shape, body.position.to_array(), body.angle, body.opacity);
                    if poses.get(&body.id) != Some(&pose) {
                        changes.push(serde_json::json!([
                            body.id,
                            shape,
                            body.position.x,
                            body.position.y,
                            body.angle,
                            body.opacity
                        ]));
                        poses.insert(body.id, pose);
                    }
                }
                let removed: Vec<_> = poses
                    .keys()
                    .filter(|id| !current.contains(id))
                    .copied()
                    .collect();
                for id in &removed {
                    poses.remove(id);
                }
                json_line(
                    &mut frames,
                    &serde_json::json!({"tick":tick,"leg":leg,"changes":changes,"removed":removed,"views":views,
                "actors":game.world().tank_poses().filter_map(|(id,position,_)|game.world().tanks().find(|t|t.id==id).map(|t|
                    serde_json::json!({"position":position,"team":t.team,"health":t.health,"mounted":t.mount.is_some(),"turret":t.turret_angle}))).collect::<Vec<_>>(),
                "shells":game.world().shells().map(|s|s.position).collect::<Vec<_>>()}),
                )?;
            }
        }
        if tick == ticks {
            break;
        }
        let start = Instant::now();
        let applied =
            std::array::from_fn::<_, ARENA_PLAYERS, _>(|i| plans[i].input_at((tick % 12) as usize));
        let events = game.step(&applied);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        step_ms.push(ms);
        let stages = game.take_physics_profile();
        profile += stages;
        if ms > 8.333 {
            slow_ticks.push(
                serde_json::json!({"tick":tick,"ms":ms,"positions":previous,"stages":stages}),
            );
        }
        for event in &events {
            if let Event::TankDestroyed { team, details, .. } = event
                && !details.mounted
            {
                sides[usize::from(*team == Team::Blue)].kills += 1;
            }
        }
        if !events.is_empty() {
            events_log.push(serde_json::json!({"tick":tick,"events":events}));
        }
        if let Some(winner) = game.world().winner() {
            sides[usize::from(winner == Team::Red)].captures += 1;
            for player in 0..ARENA_PLAYERS {
                sides[player % 2].shots += game.shots(player) as u64;
            }
            game = MultiplayerWorld::new(&level, &ARENA_TEAMS)?;
            game.configure_airdrops(true, 42);
            game.require_vehicle_choices();
            setup_profile += game.take_physics_profile();
            for (side, bot) in bots.iter_mut().enumerate() {
                let mut map = game.for_player(side).unwrap().ai_map();
                map.seed = player_seed(seed.wrapping_add(leg as u64 + 1), side);
                map.team_slot = side / 2;
                bot.reset(map)?;
            }
            previous = [None; ARENA_PLAYERS];
            health = [100.0; ARENA_PLAYERS];
            stuck_anchor = [None; ARENA_PLAYERS];
            poses.clear();
            shapes.clear();
            leg += 1;
        }
    }
    for player in 0..ARENA_PLAYERS {
        sides[player % 2].shots += game.shots(player) as u64;
    }
    let rank = |s: &Side| {
        (
            s.captures,
            s.kills,
            (s.damage * 10.0).round() as i64,
            -(s.closest_flag * 100.0).round() as i64,
        )
    };
    let winner = match rank(&sides[0]).cmp(&rank(&sides[1])) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(1),
        _ => None,
    };
    let summary = serde_json::json!({"recorded":record,"respawn_seconds":sim::RESPAWN_SECONDS,"players_per_team":2,"seed":seed,"seconds":seconds,"ticks":ticks,"decision_hz":10,"level":level.name,"versions":[bots[0].config,bots[1].config],
        "winner":winner,"scoring":"captures, player kills, observed damage, closest enemy flag (lexicographic)","sides":sides,
        "timing_ms":{"step":timing(&step_ms),"observation":timing(&observation_ms),"bot_cpu":timing(&bot_ms),"bot_round_trip":timing(&ipc_ms)},
        "physics_stage_totals_ms":profile,"setup_stage_totals_ms":setup_profile,"slow_ticks":slow_ticks});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    fs::write(out.join("geometry.json"), serde_json::to_vec(&geometry)?)?;
    fs::write(out.join("events.json"), serde_json::to_vec(&events_log)?)?;
    fs::write(
        out.join("map.json"),
        serde_json::to_vec(&game.for_player(0).unwrap().ai_map())?,
    )?;
    fs::write(
        out.join("terrain.json"),
        serde_json::to_vec(&game.world().terrain())?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
