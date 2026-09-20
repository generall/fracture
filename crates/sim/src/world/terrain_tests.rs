use super::*;
use crate::{TerrainKind, TerrainPatch};

fn drive(world: &mut World, ticks: usize, left: f32, right: f32) {
    for _ in 0..ticks {
        world.step(&[Command::Tracks { left, right }]);
    }
}

fn surface(kind: TerrainKind) -> World {
    let mut w = World::new(Scene::Custom);
    w.terrain.push(TerrainPatch {
        kind,
        min: Vector::new(-12.0, -7.5),
        max: Vector::new(12.0, 7.5),
    });
    w
}

#[test]
fn ground_is_unchanged_and_road_and_sand_have_distinct_speed_and_grip() {
    let mut baseline = World::new(Scene::Custom);
    let mut ground = surface(TerrainKind::Ground);
    let mut road = surface(TerrainKind::Road);
    let mut sand = surface(TerrainKind::Sand);
    for w in [&mut baseline, &mut ground, &mut road, &mut sand] {
        drive(w, 360, 1.0, 1.0);
    }
    assert_eq!(ground.launcher_position(), baseline.launcher_position());
    assert_eq!(ground.track_speeds(), baseline.track_speeds());
    assert!(road.track_speeds()[0] > ground.track_speeds()[0] * 1.3);
    assert!(sand.track_speeds()[0] < ground.track_speeds()[0] * 0.6);
    // Equal lateral disturbance: road grip must remove more drift in one tick.
    for w in [&mut ground, &mut road] {
        let h = w.pieces[w.launcher].body;
        w.physics.bodies[h].set_linvel(Vector::new(0.0, 3.0), true);
        w.step(&[]);
    }
    let speed = |w: &World| w.physics.bodies[w.pieces[w.launcher].body].linvel().y;
    assert!(speed(&road) < speed(&ground));
}

#[test]
fn water_and_cliffs_block_full_vehicle_at_speed_and_diagonal_approaches() {
    for kind in [TerrainKind::Water, TerrainKind::Cliff] {
        for angle in [-0.45_f32, 0.0, 0.45] {
            let mut w = World::new(Scene::Terrain);
            for patch in &mut w.terrain {
                if patch.kind == TerrainKind::Water {
                    patch.kind = kind;
                }
            }
            w.rebuild_terrain_barriers();
            w.configure_test_tank_angle(4500.0, Vector::new(-7.0, -4.2), 12.0, angle)
                .unwrap();
            drive(&mut w, 480, 1.0, 1.0);
            assert!(w.tank_alive());
            assert!(
                w.launcher_position().x < -2.3,
                "{angle}: {:?}",
                w.launcher_position()
            );
            // Tight clearance, allowing the contact solver's millimetre slop.
            let pose = w.piece_pose(w.launcher);
            for x in [-1.08, 1.08] {
                for y in [-0.82, 0.82] {
                    let p = pose.transform_point(Vector::new(x, y));
                    if p.y.abs() > 1.5 {
                        assert!(p.x < -2.28, "footprint {p:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn intact_bridge_is_crossable_in_both_directions_without_contact_damage() {
    for direction in [-1.0, 1.0] {
        let mut w = World::new(Scene::Terrain);
        w.configure_test_tank(4500.0, Vector::new(-7.0 * direction, 0.0), 4.6)
            .unwrap();
        let h = w.pieces[w.launcher].body;
        w.physics.bodies[h].set_linvel(Vector::ZERO, true);
        drive(&mut w, 500, direction, direction);
        assert!(
            w.launcher_position().x * direction > 5.0,
            "{:?}",
            w.launcher_position()
        );
        assert!(w.bridges().all(|b| b.intact));
        assert!(w.tank_alive());
        assert_eq!(w.broken, 0);
    }
}

#[test]
fn booths_survive_pushing_slow_bumps_and_glancing_impacts_from_either_bank() {
    for direction in [-1.0, 1.0] {
        for (speed, start_x, y) in [
            (0.0, -4.49, 2.1),
            (2.0, -7.0, 2.1),
            (3.0, -7.0, 2.1),
            (4.6, -7.0, 1.0),
        ] {
            let mut w = World::new(Scene::Terrain);
            w.configure_test_tank(4500.0, Vector::new(start_x * direction, y), speed)
                .unwrap();
            let h = w.pieces[w.launcher].body;
            w.physics.bodies[h].set_linvel(Vector::new(speed * direction, 0.0), true);
            if speed == 0.0 {
                w.tank_speed_limit = 4.6;
            }
            drive(&mut w, 1200, direction, direction);
            assert!(
                w.bridges().next().unwrap().intact,
                "direction {direction}, speed {speed}, y {y}: {} cracks",
                w.broken
            );
            assert!(w.tank_alive());
            assert!(
                w.launcher_position().x * direction < -4.0,
                "booth must physically stop the tank"
            );
        }
    }
}

#[test]
fn shells_and_ramming_can_remove_bridge_support() {
    for ram in [false, true] {
        let mut w = World::new(Scene::Terrain);
        w.configure_test_tank(4500.0, Vector::new(-7.0, 2.1), 4.6)
            .unwrap();
        for tick in 0..960 {
            let commands = if ram {
                vec![Command::Tracks {
                    left: 1.0,
                    right: 1.0,
                }]
            } else if tick % 60 == 0 {
                vec![Command::Fire(Shot::new(
                    Ammunition::HighExplosive,
                    Vector::new(-3.0, 2.05),
                ))]
            } else {
                vec![]
            };
            w.step(&commands);
            if !w.bridges().next().unwrap().intact {
                break;
            }
        }
        assert!(
            !w.bridges().next().unwrap().intact,
            "ram {ram}, broken {}, x {}",
            w.broken,
            w.launcher_position().x
        );
        assert!(w.broken > 0);
        assert!(w.tank_alive(), "tank on the bank must survive");
    }
}

#[test]
fn all_weapons_pass_over_the_deck_from_both_banks() {
    for direction in [-1.0, 1.0] {
        for ammo in Ammunition::ALL {
            let mut w = World::new(Scene::Terrain);
            w.configure_test_tank(4500.0, Vector::new(-7.0 * direction, 0.0), 0.0)
                .unwrap();
            w.add_box(
                Vector::new(9.0 * direction, 0.0),
                Vector::splat(0.5),
                Material::Anchor,
                true,
            );
            let mut events = w.step(&[Command::Fire(Shot::new(
                ammo,
                Vector::new(9.0 * direction, 0.0),
            ))]);
            assert_eq!(w.shots, 1, "muzzle must clear the deck");
            if ammo == Ammunition::Laser {
                assert!((w.laser_beam().unwrap().end.x * direction - 8.5).abs() < 0.01);
            } else {
                for _ in 0..if ammo == Ammunition::Rocket { 240 } else { 30 } {
                    events.extend(w.step(&[]));
                }
                let impact = events
                    .iter()
                    .find_map(|e| match e {
                        Event::Exploded { position, .. } | Event::Impacted { position, .. } => {
                            Some(position.x * direction)
                        }
                        _ => None,
                    })
                    .expect("round must hit the object beyond the bridge");
                assert!((impact - 8.5).abs() < 0.05, "{ammo:?}: {impact}");
                assert!(w.shells.is_empty());
            }
            assert_eq!(w.broken, 0);
            assert_eq!(w.ablated, 0);
            assert_eq!(w.refinements, 0);
            assert!(w.bridges().next().unwrap().intact);
        }
    }
}

#[test]
fn blasts_over_the_deck_do_not_apply_loads_to_it() {
    let mut w = World::new(Scene::Terrain);
    let mut events = vec![];
    let impact = w.next_impact();
    // Reach the deck but not either booth. No deck collider should intercept
    // pressure rays, absorb their energy or receive a fracture load.
    w.blast(Vector::ZERO, 800_000.0, 1.0, impact, &mut events);
    assert!(w.blast_pulses.is_empty());
    drive(&mut w, 120, 0.0, 0.0);
    assert_eq!(w.broken, 0);
    assert_eq!(w.refinements, 0);
    assert!(w.bridges().next().unwrap().intact);
}

#[test]
fn collapsing_bridge_sinks_occupant_once_and_closes_crossing() {
    let mut w = World::new(Scene::Terrain);
    w.configure_test_tank(4500.0, Vector::ZERO, 0.0).unwrap();
    let mut sunk = 0;
    let mut collapsed = 0;
    // Fire from the deck at the exposed bearing. Turret must clear its own deck.
    for tick in 0..1200 {
        let commands = if tick % 60 == 0 {
            vec![Command::Fire(Shot::new(
                Ammunition::HighExplosive,
                Vector::new(-3.0, 2.05),
            ))]
        } else {
            vec![]
        };
        for e in w.step(&commands) {
            sunk += usize::from(matches!(e, Event::TankSunk { .. }));
            collapsed += usize::from(matches!(e, Event::BridgeCollapsed { .. }));
        }
    }
    assert_eq!(collapsed, 1);
    assert_eq!(sunk, 1);
    assert!(!w.tank_alive());
    assert_eq!(w.terrain_at(Vector::ZERO), TerrainKind::Water);
    let position = w.launcher_position();
    let shots = w.shots;
    for _ in 0..120 {
        w.step(&[
            Command::Tracks {
                left: 1.0,
                right: -1.0,
            },
            Command::Fire(Shot::new(Ammunition::Laser, Vector::X * 10.0)),
        ]);
    }
    assert_eq!(w.launcher_position(), position);
    assert_eq!(w.shots, shots);
    assert_eq!(w.tank_fall_progress(), 1.0);
    assert_eq!(w.bridges().next().unwrap().fall_progress, 1.0);
    assert_eq!(
        w.bodies()
            .find(|b| b.material == Material::Launcher)
            .unwrap()
            .opacity,
        0.0
    );
    assert!(World::new(Scene::Terrain).tank_alive());
}

#[test]
fn terrain_fracture_and_collapse_replay_deterministically() {
    let mut a = World::new(Scene::Terrain);
    let mut b = World::new(Scene::Terrain);
    for tick in 0..420 {
        let commands = if tick % 60 == 0 {
            vec![Command::Fire(Shot::new(
                Ammunition::ArmorPiercing,
                Vector::new(-3.0, 2.05),
            ))]
        } else {
            vec![]
        };
        a.step(&commands);
        b.step(&commands);
    }
    assert!(
        !a.bridges().next().unwrap().intact,
        "repeated booth impacts must eventually sever the support"
    );
    assert_eq!(a.broken, b.broken);
    assert_eq!(
        a.bridges().next().unwrap().intact,
        b.bridges().next().unwrap().intact
    );
    assert_eq!(a.launcher_position(), b.launcher_position());
    let poses = |w: &World| {
        w.bodies()
            .map(|p| (p.id, p.position, p.angle, p.shape_revision))
            .collect::<Vec<_>>()
    };
    assert_eq!(poses(&a), poses(&b));
}

#[test]
fn a_local_cut_preserves_crossing_but_two_separately_anchored_halves_do_not() {
    let mut w = World::new(Scene::Terrain);
    let cell = w.pieces.iter().position(|p| p.bridge_deck).unwrap();
    let origin = w.pieces[cell].origin;
    let mut target = cell;
    while w.pieces[target].shape.area() > refinement::LASER_TILE_AREA {
        let next = w.pieces.len();
        assert!(w.laser_cut(target));
        target = next;
    }
    assert!(w.laser_cut(target));
    w.update_bridges(0.0, &mut vec![]);
    assert_eq!(w.ablated, 1);
    assert!(
        w.bridges().next().unwrap().intact,
        "a small edge chip is not span failure"
    );
    for p in w.pieces.iter().filter(|p| p.active && p.origin == origin) {
        assert!(p.bridge_deck);
        assert_eq!(
            w.physics.colliders[p.collider].collision_groups(),
            InteractionGroups::none()
        );
    }
    // Sever the material graph across the middle. Each half still has a bank
    // attachment, but neither carries a continuous path between both bearings.
    let poses: Vec<_> = w
        .pieces
        .iter()
        .enumerate()
        .map(|(id, p)| {
            if p.active {
                w.piece_pose(id).translation
            } else {
                Vector::ZERO
            }
        })
        .collect();
    for b in &mut w.bonds {
        if b.active && (poses[b.a].x < 0.0) != (poses[b.b].x < 0.0) {
            b.active = false;
            w.broken += 1;
        }
    }
    w.rebuild_chunks();
    assert!(
        w.chunk_members()
            .values()
            .filter(|ids| w.pieces[ids[0]].object == Some(0))
            .all(|ids| w.physics.bodies[w.pieces[ids[0]].body].is_fixed())
    );
    w.update_bridges(0.0, &mut vec![]);
    assert!(!w.bridges().next().unwrap().intact);
    assert!(w.tank_alive());
    assert!(!w.pieces.iter().any(|p| p.active && p.bridge_deck));
    drive(&mut w, 480, 1.0, 1.0);
    assert!(
        w.launcher_position().x < -3.35,
        "closed crossing must stop the hull on the bank"
    );
}

#[test]
fn water_and_cliff_collision_does_not_intercept_shells_or_laser() {
    for kind in [TerrainKind::Water, TerrainKind::Cliff] {
        for ammo in [Ammunition::HighExplosive, Ammunition::Laser] {
            let mut w = World::new(Scene::Terrain);
            for patch in &mut w.terrain {
                if patch.kind == TerrainKind::Water {
                    patch.kind = kind;
                }
            }
            w.rebuild_terrain_barriers();
            w.configure_test_tank(4500.0, Vector::new(-7.0, -4.0), 0.0)
                .unwrap();
            w.add_box(
                Vector::new(6.0, -4.0),
                Vector::splat(0.5),
                Material::Anchor,
                true,
            );
            let mut events = w.step(&[Command::Fire(Shot::new(ammo, Vector::new(6.0, -4.0)))]);
            if ammo == Ammunition::Laser {
                assert!((w.laser_beam().unwrap().end.x - 5.5).abs() < 0.01);
            } else {
                for _ in 0..20 {
                    events.extend(w.step(&[]));
                }
                let hit = events
                    .iter()
                    .find_map(|e| {
                        if let Event::Exploded { position, .. } = e {
                            Some(position.x)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                assert!((hit - 5.5).abs() < 0.05, "hit {hit}");
            }
        }
    }
}

#[test]
fn cliff_level_preserves_sight_and_routes_tanks_around_the_gap() {
    use crate::level::{Level, LevelItem, Team};
    let mut level = Level {
        size: [30.0, 20.0],
        items: vec![
            LevelItem::Spawn {
                team: Team::Blue,
                position: [-7.0, 0.0],
                angle: 0.0,
            },
            LevelItem::Terrain {
                terrain: TerrainKind::Cliff,
                min: [-1.0, -3.0],
                max: [1.0, 3.0],
            },
            // Even grass painted later cannot cover an impassable gap.
            LevelItem::Terrain {
                terrain: TerrainKind::TallGrass,
                min: [-1.0, -3.0],
                max: [1.0, 3.0],
            },
        ],
        ..Level::default()
    };
    assert_eq!(Level::from_json(&level.to_json().unwrap()).unwrap(), level);
    let w = World::from_level(&level, Team::Blue).unwrap();
    assert_eq!(w.terrain_at(Vector::ZERO), TerrainKind::Cliff);
    let target = Vector::new(7.0, 0.0);
    assert!(w.sight_clear(w.launcher, target));
    assert!(w.player_vision(0.0).contains(target));
    let route = w.tank_route(w.launcher, w.launcher_position(), target);
    assert!(!route.is_empty());
    assert!(
        route.iter().any(|p| p.y.abs() > 4.0),
        "route must go around the cliff with hull clearance"
    );
    level.items[0] = LevelItem::Spawn {
        team: Team::Blue,
        position: [0.0, 0.0],
        angle: 0.0,
    };
    assert!(
        World::from_level(&level, Team::Blue).is_err(),
        "spawning on a cliff is invalid"
    );
}
