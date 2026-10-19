use super::*;
use crate::{
    level::{Level, LevelItem},
    multiplayer::MultiplayerWorld,
};

fn config() -> Config {
    Config {
        task_control: true,
        combined_arms: true,
        speed: 1.0,
        ..Default::default()
    }
}

#[test]
fn actual_spawn_connects_to_the_route_graph_through_a_clear_corridor() {
    let level = Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
    let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
    game.require_vehicle_choices();
    let input = [PlayerInput {
        deploy: Some(VehicleKind::Tank),
        ..Default::default()
    }; 2];
    for _ in 0..3 {
        game.step(&input);
    }
    let o = game.for_player(0).unwrap().ai_observation();
    let map = game.for_player(0).unwrap().ai_map();
    let goal = map.flags.iter().find(|(t, _)| *t == Team::Red).unwrap().1 + Vec2::X * 14.0;
    let mut nav = Navigation::new(map);
    let Plan::Complete(path) = nav.plan(o.position, goal, o.vehicle) else {
        panic!("spawn disconnected from route graph");
    };
    assert!(
        nav.corridor(o.position, path[0], o.vehicle),
        "initial graph connection cuts scenery"
    );
    assert_eq!(path.last().copied(), Some(goal));
}

#[test]
fn incomplete_route_is_a_failure_and_never_a_movement_target() {
    let mut map = super::super::tests::map();
    map.layout = vec![1; map.width() * map.height()];
    for i in 0..map.layout.len() {
        if map.point(i).x.abs() < 1.0 {
            map.layout[i] = 3;
        }
    }
    let mut nav = Navigation::new(map.clone());
    assert!(matches!(
        nav.plan(
            Vec2::new(-10.0, 0.0),
            Vec2::new(10.0, 0.0),
            VehicleKind::Tank
        ),
        Plan::Unreachable { .. }
    ));
    let mut c = Controller::new(config(), map);
    c.set_patrol_goal(Vec2::new(10.0, 0.0));
    let mut o = super::super::tests::observation();
    o.position = Vec2::new(-10.0, 0.0);
    for tick in (0..600).step_by(12) {
        o.tick = tick;
        assert_eq!(c.decide(&o).tracks, [0.0; 2]);
        assert_eq!(c.telemetry.route_status, "unreachable");
    }
    assert_eq!(
        c.telemetry.replans, 1,
        "must not retry a known impossible route at 10 Hz"
    );
}

#[test]
fn new_executive_keeps_the_human_reaction_delay() {
    let mut c = Controller::new(config(), super::super::tests::map());
    let mut o = super::super::tests::observation();
    o.targets.push(Target {
        position: Vec2::new(10.0, 0.0),
        mounted: false,
        vehicle: Some(VehicleKind::Tank),
    });
    for tick in [0, 12, 24] {
        o.tick = tick;
        assert!(!c.decide(&o).fire);
    }
    o.tick = 36;
    assert!(c.decide(&o).fire);
}

#[test]
fn aiming_cannot_replace_the_selected_movement_command() {
    let mut brain = Brain::new(super::super::tests::map(), 1.0, true, true);
    let mut o = super::super::tests::observation();
    o.targets.push(Target {
        position: Vec2::new(8.0, 0.0),
        mounted: false,
        vehicle: Some(VehicleKind::Mortar),
    });
    let tracks = [0.31, -0.12];
    assert_eq!(
        brain
            .fire_control(&o, Task::Travel(Vec2::new(0.0, 10.0)), tracks)
            .tracks,
        tracks
    );
}

#[test]
fn lead_uses_consecutive_sightings_and_resets_after_target_disappears() {
    let mut brain = Brain::new(super::super::tests::map(), 1.0, true, true);
    let mut o = super::super::tests::observation();
    o.targets = vec![Target {
        position: Vec2::new(10.0, 0.0),
        mounted: false,
        vehicle: Some(VehicleKind::Tank),
    }];
    brain.prepare_fire(&o);
    assert_eq!(brain.fire_solution.unwrap().1, o.targets[0].position);
    o.tick = 12;
    o.targets[0].position.y = 0.4;
    brain.prepare_fire(&o);
    assert!(brain.fire_solution.unwrap().1.y > 0.9);
    o.tick = 24;
    o.targets.clear();
    brain.prepare_fire(&o);
    assert!(brain.fire_solution.is_none());
    o.tick = 36;
    o.targets = vec![Target {
        position: Vec2::new(10.0, 4.0),
        mounted: false,
        vehicle: Some(VehicleKind::Tank),
    }];
    brain.prepare_fire(&o);
    assert_eq!(brain.fire_solution.unwrap().1, o.targets[0].position);
}

#[test]
fn steady_tracking_does_not_require_the_stationary_reticle_floor() {
    let mut brain = Brain::new(super::super::tests::map(), 1.0, true, true);
    let mut o = super::super::tests::observation();
    o.aim_spread = 7.0_f32.to_radians();
    o.targets = vec![Target {
        position: Vec2::new(20.0, 0.0),
        mounted: false,
        vehicle: Some(VehicleKind::Scout),
    }];
    brain.turret_rate = 0.55;
    brain.prepare_fire(&o);
    assert!(
        brain
            .fire_control(&o, Task::Engage(o.targets[0].position), [0.0; 2])
            .fire
    );
    o.aim_spread = 25.0_f32.to_radians();
    assert!(
        !brain
            .fire_control(&o, Task::Engage(o.targets[0].position), [0.0; 2])
            .fire
    );
}

#[test]
fn aim_plan_is_legal_and_cannot_fire_before_its_last_sample() {
    let mut brain = Brain::new(super::super::tests::map(), 1.0, true, true);
    brain.kind = Some(VehicleKind::Tank);
    let mut input = PlayerInput {
        aim: Vec2::ZERO,
        fire: true,
        ..Default::default()
    };
    brain.aim_path(input);
    input.aim = Vec2::new(12.0, 6.0);
    let plan = InputPlan {
        input,
        aim_path: brain.aim_path(input),
    };
    assert!(plan.valid());
    assert_eq!(plan.aim_path.len(), 12);
    for step in 0..11 {
        assert!(!plan.input_at(step).fire);
    }
    assert!(plan.input_at(11).fire);
    assert_eq!(plan.input_at(11).aim, input.aim);
    let legacy = InputPlan {
        input,
        aim_path: vec![],
    };
    assert_eq!(legacy.input_at(0).aim, input.aim);
    assert!(legacy.input_at(0).fire);
    let mut invalid = plan;
    invalid.aim_path.push(Vec2::ZERO);
    assert!(!invalid.valid());
}

#[test]
fn planned_aim_avoids_full_traverse_bursts_in_real_weapon_physics() {
    let level = Level {
        size: [80.0, 60.0],
        ..Default::default()
    };
    let mut games: Vec<_> = (0..2)
        .map(|_| {
            let mut g = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
            g.require_vehicle_choices();
            let inputs = [
                PlayerInput {
                    deploy: Some(VehicleKind::Tank),
                    ..Default::default()
                },
                PlayerInput::default(),
            ];
            for _ in 0..3 {
                g.step(&inputs);
            }
            g
        })
        .collect();
    let o = games[0].for_player(0).unwrap().ai_observation();
    let mut bot = Controller::new(config(), games[0].for_player(0).unwrap().ai_map());
    bot.decide(&o);
    let mut raw = PlayerInput::default();
    let mut plan = InputPlan::default();
    let mut spread = [0.0_f32; 2];
    let mut samples = 0;
    for tick in 0..1200 {
        if tick % 12 == 0 {
            raw.aim = o.position + Vec2::from_angle(tick as f32 / 120.0 * 0.3) * 20.0;
            plan = bot.plan_input(raw);
        }
        games[0].step(&[raw, PlayerInput::default()]);
        games[1].step(&[plan.input_at((tick % 12) as usize), PlayerInput::default()]);
        if tick >= 600 && tick % 12 == 0 {
            for i in 0..2 {
                spread[i] += games[i].for_player(0).unwrap().ai_observation().aim_spread;
            }
            samples += 1;
        }
    }
    let degrees = spread.map(|v| (v / samples as f32).to_degrees());
    assert!(
        degrees[1] + 2.0 < degrees[0],
        "10 Hz steps vs planned motion: {degrees:?} degrees"
    );
}

#[test]
fn every_chassis_can_stop_aim_and_defeat_a_visible_stationary_tank() {
    for kind in VehicleKind::ALL {
        let mut level = Level {
            size: [80.0, 60.0],
            ..Default::default()
        };
        set_spawn(&mut level, Team::Blue, [-10.0, -5.0], 0.25);
        set_spawn(&mut level, Team::Red, [10.0, -5.0], std::f32::consts::PI);
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [
            PlayerInput {
                deploy: Some(kind),
                ..Default::default()
            },
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bot = Controller::new(config(), game.for_player(0).unwrap().ai_map());
        let mut defeated = false;
        for tick in 0..1800 {
            if tick % 12 == 0 {
                let o = game.for_player(0).unwrap().ai_observation();
                inputs[0] = bot.decide(&o);
                if tick < 36 {
                    assert!(!inputs[0].fire, "{kind:?}: bypassed sight delay");
                }
            }
            game.step(&inputs);
            if !game.for_player(1).unwrap().ai_observation().alive {
                defeated = true;
                break;
            }
        }
        assert!(
            defeated,
            "{kind:?} failed visible combat: {:?}, shots {}",
            bot.telemetry,
            game.shots(0)
        );
    }
}

#[test]
fn scout_interception_mission_leaves_the_breach_route_to_heavies() {
    let level = Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
    let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
    game.require_vehicle_choices();
    let input = [
        PlayerInput {
            deploy: Some(VehicleKind::Scout),
            ..Default::default()
        },
        PlayerInput::default(),
    ];
    for _ in 0..3 {
        game.step(&input);
    }
    let mut brain = Brain::new(game.for_player(0).unwrap().ai_map(), 1.0, true, true);
    let o = game.for_player(0).unwrap().ai_observation();
    brain.nav.observe(&o);
    brain.mission_task(&o, None);
    let m = brain.mission.as_ref().expect("reachable scout mission");
    assert!(
        m.station.distance(m.objective) > 60.0,
        "scout still ordered to the enemy wall"
    );
    assert!(brain.nav.public_safe(m.station));
}

#[test]
fn a_heavy_has_a_complete_perimeter_route_on_both_colours() {
    for team in [Team::Blue, Team::Red] {
        let level =
            Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
        let mut game = MultiplayerWorld::new(&level, &[team, team.other()]).unwrap();
        game.require_vehicle_choices();
        let inputs = [
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
            PlayerInput::default(),
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let o = game.for_player(0).unwrap().ai_observation();
        let mut brain = Brain::new(game.for_player(0).unwrap().ai_map(), 1.0, true, true);
        brain.use_perimeter(true);
        brain.nav.observe(&o);
        brain.mission_task(&o, None);
        let m = brain.mission.as_ref().expect("reachable base approach");
        assert!(
            m.via.is_some(),
            "{team:?}: perimeter silently fell back to central route"
        );
        assert!(m.phase == Phase::Flank);
        let via = m.via.unwrap();
        assert!(o.position.distance(via) > 30.0);
        assert!(matches!(
            brain.nav.plan(via, m.station, o.vehicle),
            Plan::Complete(_)
        ));
        assert!(
            brain
                .route
                .as_ref()
                .unwrap()
                .points
                .iter()
                .all(|p| p.distance(m.station) >= via.distance(m.station) - 0.01),
            "flank turns away after forward progress"
        );
    }
}

#[test]
fn two_visible_heavies_choose_different_missions() {
    for a in [VehicleKind::Tank, VehicleKind::Mortar] {
        for b in [VehicleKind::Tank, VehicleKind::Mortar] {
            let decisions: Vec<_> = [(0, a, b), (1, b, a)]
                .into_iter()
                .map(|(slot, own, ally)| {
                    let mut map = super::super::tests::map();
                    map.team_slot = slot;
                    let brain = Brain::new(map, 1.0, true, true);
                    let mut o = super::super::tests::observation();
                    o.vehicle = own;
                    o.allied_vehicles = vec![AlliedVehicle {
                        position: Vec2::X * 3.0,
                        vehicle: ally,
                    }];
                    brain.should_flank(&o)
                })
                .collect();
            assert_ne!(
                decisions[0], decisions[1],
                "{a:?}/{b:?} chose the same mission"
            );
        }
    }
}

#[test]
fn route_lookahead_cannot_cut_across_a_public_defense_sector() {
    let mut map = super::super::tests::map();
    map.size = Vec2::splat(80.0);
    map.layout = vec![1; map.width() * map.height()];
    map.emplacements = vec![Emplacement {
        team: Team::Red,
        position: Vec2::new(0.0, -10.0),
        facing: std::f32::consts::FRAC_PI_2,
        arc: 60.0,
        range: 20.0,
    }];
    let nav = Navigation::new(map);
    let a = Vec2::new(-15.0, 0.0);
    let b = Vec2::new(15.0, 0.0);
    assert!(nav.public_safe(a) && nav.public_safe(b));
    assert!(nav.corridor(a, b, VehicleKind::Tank));
    assert!(!nav.route_corridor(a, b, VehicleKind::Tank));
}

#[test]
fn a_stationary_teammate_yields_in_real_physics() {
    let mut level = Level {
        size: [80.0, 60.0],
        ..Default::default()
    };
    set_spawn(&mut level, Team::Blue, [-8.0, 0.0], 0.0);
    set_spawn(&mut level, Team::Red, [0.0, 0.0], 0.0);
    for i in &mut level.items {
        if let LevelItem::Flag { position, .. } = i {
            *position = [20.0, 0.0];
        }
    }
    let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
    game.require_vehicle_choices();
    let mut inputs = [PlayerInput {
        deploy: Some(VehicleKind::Tank),
        ..Default::default()
    }; 2];
    for _ in 0..3 {
        game.step(&inputs);
    }
    let mut bots: Vec<_> = (0..2)
        .map(|i| {
            let mut map = game.for_player(i).unwrap().ai_map();
            map.team = Team::Blue;
            map.team_slot = i;
            let mut c = Controller::new(config(), map);
            c.set_patrol_goal(if i == 0 {
                Vec2::new(10.0, 0.0)
            } else {
                Vec2::ZERO
            });
            c
        })
        .collect();
    // The lower-priority tank has reached its firing station and stays put
    // unless traffic can interrupt a stationary task. All movement is physical.
    let mut passed = false;
    for tick in 0..3600 {
        if tick % 12 == 0 {
            let obs: Vec<_> = (0..2)
                .map(|i| game.for_player(i).unwrap().ai_observation())
                .collect();
            for i in 0..2 {
                let mut o = obs[i].clone();
                let ally = &obs[1 - i];
                if o.targets
                    .iter()
                    .any(|t| t.position.distance(ally.position) < 1.0)
                {
                    o.allied_vehicles.push(AlliedVehicle {
                        position: ally.position,
                        vehicle: ally.vehicle,
                    });
                    o.allies.push(ally.position);
                }
                o.targets.clear();
                inputs[i] = bots[i].decide(&o);
                inputs[i].fire = false;
                inputs[i].special = false;
            }
            if obs[0].position.x > 7.0 {
                passed = true;
                break;
            }
        }
        game.step(&inputs);
    }
    assert!(
        passed,
        "stationary teammate blocked right of way: {:?} / {:?}",
        bots[0].telemetry, bots[1].telemetry
    );
}

#[test]
fn river_and_spawn_routes_are_complete_physics_behaviours() {
    for kind in VehicleKind::ALL {
        for (start, goal, angle) in [
            ([15.0, -23.0], [-15.0, 25.0], 2.2),
            ([-15.0, 25.0], [15.0, -23.0], -0.9),
            (
                [43.3536, -73.3322],
                [15.0, -23.0],
                std::f32::consts::FRAC_PI_2,
            ),
        ] {
            let mut level =
                Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
            level
                .items
                .retain(|i| !matches!(i, LevelItem::Turret { .. } | LevelItem::Pickup { .. }));
            set_spawn(&mut level, Team::Blue, start, angle);
            let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
            game.require_vehicle_choices();
            let mut input = [
                PlayerInput {
                    deploy: Some(kind),
                    ..Default::default()
                },
                PlayerInput::default(),
            ];
            for _ in 0..3 {
                game.step(&input);
            }
            let mut c = Controller::new(config(), game.for_player(0).unwrap().ai_map());
            c.set_patrol_goal(goal.into());
            let mut reached = false;
            for tick in 0..10800 {
                if tick % 12 == 0 {
                    let o = game.for_player(0).unwrap().ai_observation();
                    assert_eq!(
                        o.health, 100.0,
                        "{kind:?} {start:?}: damage at {:?}",
                        o.position
                    );
                    if o.position.distance(goal.into()) < 2.0 {
                        reached = true;
                        break;
                    }
                    input[0] = c.decide(&o);
                    input[0].fire = false;
                    input[0].special = false;
                    trace("river", tick, &o, &c, &input[0]);
                }
                game.step(&input);
            }
            assert!(
                reached,
                "{kind:?} {start:?}: stopped at {:?}, {:?}",
                game.for_player(0).unwrap().launcher_position(),
                c.telemetry
            );
        }
    }
}

#[test]
fn teammates_pass_the_rocks_north_of_the_blue_base() {
    let mut level =
        Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
    level
        .items
        .retain(|i| !matches!(i, LevelItem::Turret { .. } | LevelItem::Pickup { .. }));
    set_spawn(&mut level, Team::Red, [74.64821, -36.65189], -1.3254);
    set_spawn(&mut level, Team::Blue, [76.9847, -39.373333], 0.9955);
    let mut game = MultiplayerWorld::new(&level, &[Team::Red, Team::Blue]).unwrap();
    game.require_vehicle_choices();
    let mut inputs = [
        PlayerInput {
            deploy: Some(VehicleKind::Tank),
            ..Default::default()
        },
        PlayerInput {
            deploy: Some(VehicleKind::Scout),
            ..Default::default()
        },
    ];
    for _ in 0..3 {
        game.step(&inputs);
    }
    let goal = Vec2::new(65.93407, -51.406593);
    let mut bots: Vec<_> = (0..2)
        .map(|i| {
            let mut map = game.for_player(i).unwrap().ai_map();
            map.team = Team::Red;
            map.team_slot = i;
            let mut bot = Controller::new(config(), map);
            bot.set_patrol_goal(goal);
            bot
        })
        .collect();
    let mut reached = false;
    for tick in 0..7200 {
        if tick % 12 == 0 {
            let observations: Vec<_> = (0..2)
                .map(|i| game.for_player(i).unwrap().ai_observation())
                .collect();
            for i in 0..2 {
                let mut o = observations[i].clone();
                let ally = &observations[1 - i];
                if o.targets
                    .iter()
                    .any(|t| t.position.distance(ally.position) < 1.0)
                {
                    o.allied_vehicles.push(AlliedVehicle {
                        position: ally.position,
                        vehicle: ally.vehicle,
                    });
                    o.allies.push(ally.position);
                }
                o.targets.clear();
                inputs[i] = bots[i].decide(&o);
                inputs[i].fire = false;
                inputs[i].special = false;
                trace(
                    if i == 0 { "rocks-tank" } else { "rocks-scout" },
                    tick,
                    &o,
                    &bots[i],
                    &inputs[i],
                );
            }
            if observations[0].position.distance(goal) < 2.0 {
                reached = true;
                break;
            }
        }
        game.step(&inputs);
    }
    assert!(
        reached,
        "tank {:?}, scout {:?}; {:?} / {:?}",
        game.for_player(0).unwrap().launcher_position(),
        game.for_player(1).unwrap().launcher_position(),
        bots[0].telemetry,
        bots[1].telemetry
    );
}

#[test]
fn a_scout_at_an_oblique_heading_does_not_park_in_a_teammates_lane() {
    // Regression for the real failure: the scout faces northwest while the
    // tank behind it travels north. Passing must use intended travel, not the
    // scout's heading. Run multiple headings, not one aligned happy path.
    for angle in [std::f32::consts::FRAC_PI_2, 2.4, -2.4] {
        let mut level = Level {
            size: [80.0, 60.0],
            ..Default::default()
        };
        set_spawn(
            &mut level,
            Team::Blue,
            [0.0, -12.0],
            std::f32::consts::FRAC_PI_2,
        );
        set_spawn(&mut level, Team::Red, [0.0, -3.0], angle);
        for item in &mut level.items {
            if let LevelItem::Flag { position, .. } = item {
                *position = [-25.0, 25.0];
            }
        }
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
            PlayerInput {
                deploy: Some(VehicleKind::Scout),
                ..Default::default()
            },
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bots: Vec<_> = (0..2)
            .map(|i| {
                let mut map = game.for_player(i).unwrap().ai_map();
                map.team = Team::Blue;
                map.team_slot = i;
                let mut c = Controller::new(config(), map);
                c.set_patrol_goal(Vec2::new(0.0, 20.0));
                c
            })
            .collect();
        let mut reached = false;
        for tick in 0..4800 {
            if tick % 12 == 0 {
                let observations: Vec<_> = (0..2)
                    .map(|i| game.for_player(i).unwrap().ai_observation())
                    .collect();
                for i in 0..2 {
                    let mut o = observations[i].clone();
                    let ally = &observations[1 - i];
                    if o.targets
                        .iter()
                        .any(|t| t.position.distance(ally.position) < 1.0)
                    {
                        o.allied_vehicles.push(AlliedVehicle {
                            position: ally.position,
                            vehicle: ally.vehicle,
                        });
                        o.allies.push(ally.position);
                    }
                    o.targets.clear();
                    inputs[i] = bots[i].decide(&o);
                    inputs[i].fire = false;
                    inputs[i].special = false;
                    trace(
                        if i == 0 {
                            "passing-tank"
                        } else {
                            "passing-scout"
                        },
                        tick,
                        &o,
                        &bots[i],
                        &inputs[i],
                    );
                }
                if observations[0].position.y > 17.0 {
                    reached = true;
                    break;
                }
            }
            game.step(&inputs);
        }
        assert!(
            reached,
            "heading {angle}: tank {:?}, scout {:?}; {:?}, {:?}",
            game.for_player(0).unwrap().launcher_position(),
            game.for_player(1).unwrap().launcher_position(),
            bots[0].telemetry,
            bots[1].telemetry
        );
    }
}

#[test]
fn tank_breaches_and_enters_real_bases_on_both_colours() {
    for team in [Team::Blue, Team::Red] {
        let mut level =
            Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
        level
            .items
            .retain(|i| !matches!(i, LevelItem::Turret { .. } | LevelItem::Pickup { .. }));
        let objective = level
            .items
            .iter()
            .find_map(|i| match i {
                LevelItem::Flag { team: t, position } if *t != team => Some(Vec2::from(*position)),
                _ => None,
            })
            .unwrap();
        set_spawn(
            &mut level,
            team,
            (objective + Vec2::X * 12.0).into(),
            std::f32::consts::PI,
        );
        let mut game = MultiplayerWorld::new(&level, &[team, team.other()]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
            PlayerInput::default(),
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut c = Controller::new(config(), game.for_player(0).unwrap().ai_map());
        let mut captured = false;
        for tick in 0..14400 {
            if tick % 12 == 0 {
                let o = game.for_player(0).unwrap().ai_observation();
                inputs[0] = c.decide(&o);
                trace("siege", tick, &o, &c, &inputs[0]);
            }
            game.step(&inputs);
            if game.world().winner() == Some(team) {
                captured = true;
                break;
            }
        }
        if !captured {
            let o = game.for_player(0).unwrap().ai_observation();
            let brain = c.task_brain.as_ref().unwrap();
            let samples:Vec<_>=(0..=28).map(|n| {
                let p=o.position.lerp(objective,n as f32/28.0);
                serde_json::json!({"p":p,"cell":brain.nav.cells[brain.nav.map.index(p)],"visible":o.visible(p),"clearance":o.obstacles.iter().map(|b|polygon_distance(p,&b.outline)).fold(100.0_f32,f32::min)})
            }).collect();
            eprintln!(
                "Breach diagnostic {team:?}: {}",
                serde_json::to_string(&samples).unwrap()
            );
        }
        assert!(
            captured,
            "{team:?}: stopped at {:?}; {:?}",
            game.for_player(0).unwrap().launcher_position(),
            c.telemetry
        );
    }
}

fn set_spawn(level: &mut Level, team: Team, pos: [f32; 2], heading: f32) {
    for item in &mut level.items {
        if let LevelItem::Spawn {
            team: t,
            position,
            angle,
        } = item
            && *t == team
        {
            *position = pos;
            *angle = heading;
        }
    }
}

#[test]
fn a_scout_clears_the_real_garage_before_the_traffic_timeout() {
    for team in [Team::Blue, Team::Red] {
        let mut level =
            Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
        level
            .items
            .retain(|i| !matches!(i, LevelItem::Turret { .. } | LevelItem::Pickup { .. }));
        let mut game =
            MultiplayerWorld::new(&level, &[team, team.other(), team, team.other()]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [PlayerInput::default(); 4];
        inputs[0].deploy = Some(VehicleKind::Tank);
        inputs[2].deploy = Some(VehicleKind::Scout);
        for _ in 0..3 {
            game.step(&inputs);
        }
        let start = game.for_player(0).unwrap().launcher_position();
        let mut bots: Vec<_> = [0, 2]
            .into_iter()
            .map(|i| {
                let mut map = game.for_player(i).unwrap().ai_map();
                map.team_slot = i / 2;
                map.seed = player_seed(42, i);
                Controller::new(
                    Config {
                        perimeter_routes: true,
                        ..config()
                    },
                    map,
                )
            })
            .collect();
        let mut plans = [InputPlan::default(), InputPlan::default()];
        let mut departed = false;
        // The old controller remains blocked until its 20 s passing timeout.
        // Require actual departure before that expiry, including the tank's
        // ordinary acceleration and turn through the garage exit.
        for tick in 0..2400 {
            for (slot, i) in [0, 2].into_iter().enumerate() {
                if tick % 12 == 0 {
                    let o = game.for_player(i).unwrap().ai_observation();
                    let input = bots[slot].decide(&o);
                    plans[slot] = bots[slot].plan_input(input);
                    trace(
                        if slot == 0 {
                            "garage-tank"
                        } else {
                            "garage-scout"
                        },
                        tick,
                        &o,
                        &bots[slot],
                        &input,
                    );
                }
                inputs[i] = plans[slot].input_at((tick % 12) as usize);
            }
            game.step(&inputs);
            if game
                .for_player(0)
                .unwrap()
                .launcher_position()
                .distance(start)
                > 12.0
            {
                eprintln!(
                    "garage team {team:?} departed at {:.1}s",
                    tick as f32 / 120.0
                );
                departed = true;
                break;
            }
        }
        assert!(
            departed,
            "{team:?}: tank {:?}; scout {:?}; {:?} / {:?}",
            game.for_player(0).unwrap().launcher_position(),
            game.for_player(2).unwrap().launcher_position(),
            bots[0].telemetry,
            bots[1].telemetry
        );
    }
}

#[test]
#[ignore = "full four-minute guarded River Valley team behaviour; run explicitly"]
fn team_travels_from_spawn_and_captures_a_guarded_base() {
    guarded_team_contract(false);
}

#[test]
#[ignore = "full four-minute guarded perimeter mission; run explicitly"]
fn perimeter_team_captures_a_guarded_base() {
    guarded_team_contract(true);
}

fn guarded_team_contract(perimeter: bool) {
    // A diagnostic contract, not a league match: teammate chassis are fixed,
    // opponents remain stationary, and authored defenses retain normal AI.
    for team in [Team::Blue, Team::Red] {
        let level =
            Level::from_json(include_str!("../../../../../levels/dota-level.json")).unwrap();
        let mut game =
            MultiplayerWorld::new(&level, &[team, team.other(), team, team.other()]).unwrap();
        game.require_vehicle_choices();
        let kinds = [
            VehicleKind::Tank,
            VehicleKind::Tank,
            VehicleKind::Scout,
            VehicleKind::Tank,
        ];
        let mut inputs = std::array::from_fn::<_, 4, _>(|i| PlayerInput {
            deploy: Some(kinds[i]),
            ..Default::default()
        });
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bots: Vec<_> = [0, 2]
            .into_iter()
            .map(|i| {
                let mut map = game.for_player(i).unwrap().ai_map();
                map.team_slot = i / 2;
                map.seed = player_seed(42, i);
                Controller::new(
                    Config {
                        perimeter_routes: perimeter,
                        ..config()
                    },
                    map,
                )
            })
            .collect();
        let mut captured = false;
        let mut plans: [InputPlan; 4] = std::array::from_fn(|_| InputPlan::default());
        for tick in 0..ROUND_TICKS {
            if tick % 12 == 0 {
                for (slot, i) in [0, 2].into_iter().enumerate() {
                    let o = game.for_player(i).unwrap().ai_observation();
                    inputs[i] = bots[slot].decide(&o);
                    plans[i] = bots[slot].plan_input(inputs[i]);
                    inputs[i].deploy = Some(kinds[i]);
                    trace(
                        if slot == 0 { "team-tank" } else { "team-scout" },
                        tick,
                        &o,
                        &bots[slot],
                        &inputs[i],
                    );
                }
            }
            for i in [0, 2] {
                inputs[i] = plans[i].input_at((tick % 12) as usize);
                inputs[i].deploy = Some(kinds[i]);
            }
            game.step(&inputs);
            if game.world().winner() == Some(team) {
                eprintln!(
                    "guarded team {team:?} perimeter={perimeter} capture at {:.1}s",
                    tick as f32 / 120.0
                );
                captured = true;
                break;
            }
        }
        if !captured && std::env::var_os("FRACTURE_AI_CONTRACT_TRACE").is_some() {
            let o = game.for_player(0).unwrap().ai_observation();
            eprintln!(
                "{}",
                serde_json::json!({"failure": "guarded-team", "team": team, "observation": o})
            );
        }
        assert!(
            captured,
            "{team:?}: tank {:?}, scout {:?}; {:?} / {:?}",
            game.for_player(0).unwrap().launcher_position(),
            game.for_player(2).unwrap().launcher_position(),
            bots[0].telemetry,
            bots[1].telemetry
        );
    }
}

fn trace(case: &str, tick: u32, o: &Observation, c: &Controller, input: &PlayerInput) {
    if tick.is_multiple_of(120) && std::env::var_os("FRACTURE_AI_CONTRACT_TRACE").is_some() {
        eprintln!(
            "{}",
            serde_json::json!({"case":case,"tick":tick,"position":o.position,"hull":o.hull_angle,"health":o.health,"targets":o.targets,"telemetry":c.telemetry,"input":input})
        );
    }
}
