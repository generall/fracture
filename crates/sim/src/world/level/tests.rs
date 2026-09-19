use super::*;
use crate::{Ammunition, LibraryPreset};
fn fixture() -> Level {
    Level {
        size: [24.0, 15.0],
        items: vec![
            LevelItem::Spawn {
                team: Team::Blue,
                position: [-7.0, 0.0],
                angle: 0.0,
            },
            LevelItem::Spawn {
                team: Team::Red,
                position: [7.0, 0.0],
                angle: std::f32::consts::PI,
            },
            LevelItem::Flag {
                team: Team::Blue,
                position: [-10.0, 0.0],
            },
            LevelItem::Flag {
                team: Team::Red,
                position: [10.0, 0.0],
            },
        ],
        ..Level::default()
    }
}

#[test]
fn level_walls_and_fences_attach_to_authored_solids_without_generated_blocks() {
    for make_asset in [crate::level::wall_asset, crate::level::fence_asset] {
        for angle in [0.0, 0.6] {
            for gap in [-2.0, -0.2, 0.0, 0.2, 0.3] {
                for support_first in [false, true] {
                    let mut asset = make_asset(Vector::new(1.0, 6.0));
                    // Earlier drafts must lose their automatically generated pads too.
                    asset.pinned_edge = Some(2);
                    asset.support_spacing = Some(4.0);
                    let wall = LevelItem::Object {
                        cutouts: vec![],
                        asset: Box::new(asset),
                        position: [0.0, 0.0],
                        angle,
                    };
                    let support = LevelItem::Solid {
                        cutouts: vec![],
                        outline: crate::level::rectangle(Vector::splat(0.6)),
                        position: Vector::from_angle(angle)
                            .rotate(Vector::new(0.0, 3.6 + gap))
                            .to_array(),
                        angle,
                    };
                    let mut level = Level {
                        size: [24.0, 20.0],
                        items: vec![LevelItem::Spawn {
                            team: Team::Blue,
                            position: [-8.0, 0.0],
                            angle: 0.0,
                        }],
                        ..Level::default()
                    };
                    level.items.extend(if support_first {
                        [support, wall]
                    } else {
                        [wall, support]
                    });
                    let level = Level::from_json(&level.to_json().unwrap()).unwrap();
                    let mut world = World::from_level(&level, Team::Blue).unwrap();
                    assert_eq!(
                        world
                            .pieces
                            .iter()
                            .filter(|p| p.material == Material::Anchor)
                            .count(),
                        2,
                        "only the two authored solid triangles should be anchor pieces"
                    );
                    let joints: Vec<_> = world
                        .bonds
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| {
                            world.pieces[b.a].material == Material::Anchor
                                || world.pieces[b.b].material == Material::Anchor
                        })
                        .map(|(i, _)| i)
                        .collect();
                    assert_eq!(!joints.is_empty(), gap <= 0.25, "gap {gap}, angle {angle}");
                    assert!(
                        world
                            .bodies()
                            .filter(|b| b.material == Material::Masonry)
                            .all(|b| b.fixed == (gap <= 0.25))
                    );
                    for i in joints {
                        world.bonds[i].active = false;
                    }
                    world.rebuild_chunks();
                    assert!(
                        world
                            .bodies()
                            .filter(|b| b.material == Material::Masonry)
                            .all(|b| !b.fixed),
                        "breaking the connection must release the wall"
                    );
                    world.step(&[]);
                    assert!(
                        world
                            .bodies()
                            .filter(|b| b.material == Material::Masonry)
                            .all(|b| !b.fixed),
                        "broken connections must not regenerate during play"
                    );
                }
            }
        }
    }
}

#[test]
fn carved_embedded_support_does_not_inject_contact_energy() {
    let saved = Level::from_json(include_str!(
        "../../../tests/fixtures/embedded-wall-impact.json"
    ))
    .unwrap();
    let mut wall = saved.items[7].clone();
    let mut support = saved.items[4].clone();
    for item in [&mut wall, &mut support] {
        item.translate(Vector::new(11.75, 3.25));
    }
    let level = Level {
        size: [24.0, 20.0],
        items: vec![
            LevelItem::Spawn {
                team: Team::Blue,
                position: [-5.0, 0.0],
                angle: 0.0,
            },
            wall,
            support,
        ],
        ..Level::default()
    };
    for speed in [-1.0, 0.2, 0.5, 1.0] {
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        if speed < 0.0 {
            for b in &mut world.bonds {
                if world.pieces[b.a].material == Material::Anchor {
                    b.active = false;
                }
            }
            world.rebuild_chunks();
        } else {
            world
                .configure_test_tank(4500.0, Vector::new(-1.34, 0.0), speed)
                .unwrap();
        }
        let mut peak = 0.0_f32;
        let mut kinetic = 0.0_f32;
        for _ in 0..240 {
            world.step(&[]);
            peak = peak.max(
                world
                    .bodies()
                    .filter(|b| b.material == Material::Masonry)
                    .map(|b| b.velocity.length())
                    .fold(0.0, f32::max),
            );
            kinetic = kinetic.max(
                world
                    .physics
                    .bodies
                    .iter()
                    .filter(|(_, b)| b.is_dynamic())
                    .map(|(_, b)| b.kinetic_energy())
                    .sum::<f32>(),
            );
        }
        if speed < 0.0 {
            assert_eq!(
                world.stats().broken,
                0,
                "releasing supports at rest must not fracture material"
            );
            assert!(
                peak < 0.001 && kinetic < 0.001,
                "overlap must not inject separation energy"
            );
        } else {
            let initial_energy = 0.5 * 4500.0 * speed * speed;
            assert!(
                kinetic <= initial_energy * 1.05,
                "contact created energy: {kinetic} > {initial_energy}"
            );
            if speed == 0.2 {
                assert_eq!(world.stats().broken, 0);
            }
            if speed == 0.5 {
                assert!(
                    peak < 0.5,
                    "gentle impact launched a fragment at {peak} m/s"
                );
            }
        }
    }
}

#[test]
fn saved_editor_map_embedded_blocks_anchor_both_wall_sections() {
    let saved = Level::from_json(include_str!(
        "../../../tests/fixtures/embedded-wall-supports.json"
    ))
    .unwrap();
    for rotation in [0.0, 0.37] {
        let mut level = saved.clone();
        for item in &mut level.items {
            if let LevelItem::Solid { angle, .. } = item {
                *angle += rotation;
            }
        }
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        let anchors: Vec<_> = world
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.material == Material::Anchor)
            .map(|(id, _)| id)
            .collect();
        assert_eq!(anchors.len(), 6, "only the three authored blocks exist");
        for &id in &anchors {
            if world.piece_pose(id).translation.x < -11.1 {
                assert!(
                    world
                        .bonds
                        .iter()
                        .any(|b| b.active && (b.a == id || b.b == id)),
                    "both embedded blocks must connect to wall material"
                );
            }
        }
        let masonry: Vec<_> = world
            .bodies()
            .filter(|b| b.material == Material::Masonry)
            .map(|b| (b.id, b.position))
            .collect();
        assert!(!masonry.is_empty());
        assert!(
            world
                .bodies()
                .filter(|b| b.material == Material::Masonry)
                .all(|b| b.fixed)
        );
        for _ in 0..120 {
            world.step(&[]);
        }
        assert_eq!(
            world.stats().broken,
            0,
            "embedded blocks must not fracture a resting wall"
        );
        for (id, position) in masonry {
            assert!(world.piece_pose(id).translation.distance(position) < 0.001);
        }
        world.step(&[Command::Fire(Shot::new(
            Ammunition::HighExplosive,
            Vector::new(-11.5, -2.75),
        ))]);
        for _ in 0..180 {
            world.step(&[]);
        }
        assert!(
            world.stats().broken > 0,
            "supported wall remains destructible"
        );
        assert_eq!(
            world
                .pieces
                .iter()
                .filter(|p| p.material == Material::Anchor)
                .count(),
            6
        );
    }
}

#[test]
fn map_roundtrip_retains_independent_shapes_and_long_wall_grains() {
    let source = LibraryPreset::Wall.asset();
    let mut item = LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(source.clone()),
        position: [0.0, 0.0],
        angle: 0.4,
    };
    item.resize(Vector::new(22.0, 0.8)).unwrap();
    let LevelItem::Object { asset, .. } = &item else {
        unreachable!()
    };
    assert_ne!(asset.outline, source.outline);
    assert_eq!(
        asset.layout.as_ref().unwrap().size,
        source.layout.as_ref().unwrap().size
    );
    assert!(
        asset
            .level_mesh()
            .unwrap()
            .grains
            .iter()
            .copied()
            .max()
            .unwrap()
            > 20
    );
    assert!(asset.mesh().is_err(), "workshop bounds stay unchanged");
    let mut level = Level::default();
    level.items.push(item);
    let decoded = Level::from_json(&level.to_json().unwrap()).unwrap();
    assert_eq!(level, decoded);
    let w = World::from_level(&decoded, Team::Blue).unwrap();
    assert!(w.stats().cells > 40);
}
#[test]
fn invalid_maps_are_rejected_before_loading() {
    let mut l = fixture();
    l.version = 2;
    assert!(l.validate().is_err());
    l = fixture();
    l.items.push(l.items[0].clone());
    assert!(l.validate().is_err());
    l = fixture();
    l.items.push(LevelItem::Terrain {
        terrain: crate::TerrainKind::Water,
        min: [0.0, 1.0],
        max: [1.0, 0.0],
    });
    assert!(l.validate().is_err());
    l = fixture();
    l.items.push(LevelItem::Flag {
        team: Team::Red,
        position: [f32::NAN, 0.0],
    });
    assert!(l.validate().is_err());
    l = fixture();
    l.items.push(LevelItem::Solid {
        cutouts: vec![],
        outline: vec![[0.0, 0.0], [1.0, 1.0], [0.0, 1.0], [1.0, 0.0]],
        position: [0.0, 0.0],
        angle: 0.0,
    });
    assert!(l.validate().is_err());
    assert!(Level::from_json(&" ".repeat(crate::level::MAX_LEVEL_BYTES + 1)).is_err());
}
#[test]
fn spawns_use_team_heading_and_reject_hull_overlap_or_open_water() {
    let l = fixture();
    let w = World::from_level(&l, Team::Red).unwrap();
    assert_eq!(w.team(), Team::Red);
    assert_eq!(w.launcher_position(), Vector::new(7.0, 0.0));
    assert!((w.piece_pose(w.launcher).rotation.angle().abs() - std::f32::consts::PI).abs() < 1e-5);
    for obstacle in [
        LevelItem::Solid {
            cutouts: vec![],
            outline: crate::level::rectangle(Vector::splat(0.5)),
            position: [-6.0, 0.0],
            angle: 0.0,
        },
        LevelItem::Terrain {
            terrain: crate::TerrainKind::Water,
            min: [-6.1, -2.0],
            max: [-4.0, 2.0],
        },
    ] {
        let mut blocked = l.clone();
        blocked.items.push(obstacle);
        assert!(World::from_level(&blocked, Team::Blue).is_err());
    }
}
#[test]
fn flags_ignore_every_weapon_and_friendly_tank_touch() {
    for ammo in Ammunition::ALL {
        let mut l = fixture();
        l.items[2] = LevelItem::Flag {
            team: Team::Blue,
            position: [-7.0, 0.0],
        };
        l.items[3] = LevelItem::Flag {
            team: Team::Red,
            position: [0.0, 0.0],
        };
        let mut w = World::from_level(&l, Team::Blue).unwrap();
        for tick in 0..240 {
            let cmd = if tick % 60 == 0 {
                vec![Command::Fire(Shot::new(ammo, Vector::ZERO))]
            } else {
                vec![]
            };
            assert!(
                !w.step(&cmd)
                    .iter()
                    .any(|e| matches!(e, Event::FlagCaptured { .. }))
            );
        }
        assert_eq!(w.flags().count(), 2);
        assert_eq!(w.winner(), None);
        assert_eq!(w.stats().ablated, 0);
    }
}
#[test]
fn only_live_enemy_tank_footprint_captures_and_emits_one_victory() {
    for team in [Team::Blue, Team::Red] {
        let mut l = fixture();
        let position = if team == Team::Blue { -7.0 } else { 7.0 };
        l.items.retain(|i| !matches!(i, LevelItem::Flag { .. }));
        l.items.push(LevelItem::Flag {
            team: team.other(),
            position: [position, 1.3],
        });
        let mut w = World::from_level(&l, team).unwrap();
        assert_eq!(
            w.step(&[])
                .iter()
                .filter(|e| matches!(e, Event::FlagCaptured { .. }))
                .count(),
            0
        );
        w.flags[0].position.y = 1.1; // track edge at .82 plus flag radius .35
        w.tank_fall = Some(0.0);
        let mut events = vec![];
        w.capture_flags(&mut events);
        assert!(events.is_empty());
        w.tank_fall = None;
        assert_eq!(
            w.step(&[])
                .iter()
                .filter(|e| matches!(e,Event::FlagCaptured {team:t,..} if *t==team))
                .count(),
            1
        );
        assert_eq!(w.winner(), Some(team));
        let pose = w.launcher_position();
        let tick = w.stats().ticks;
        assert!(
            w.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0
            }])
            .is_empty()
        );
        assert_eq!(w.launcher_position(), pose);
        assert_eq!(w.stats().ticks, tick);
    }
}
#[test]
fn tank_can_drive_to_enemy_flag_and_restart_from_unchanged_recipe() {
    let mut l = fixture();
    l.items[3] = LevelItem::Flag {
        team: Team::Red,
        position: [-2.0, 0.0],
    };
    let mut w = World::from_level(&l, Team::Blue).unwrap();
    for _ in 0..480 {
        w.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
    }
    assert_eq!(w.winner(), Some(Team::Blue));
    let reset = World::from_level(&l, Team::Blue).unwrap();
    assert_eq!(reset.winner(), None);
    assert_eq!(reset.launcher_position(), Vector::new(-7.0, 0.0));
}
#[test]
fn indestructible_polygon_blocks_tanks_and_survives_all_ammunition() {
    let mut l = fixture();
    l.items.push(LevelItem::Solid {
        cutouts: vec![],
        outline: crate::level::rectangle(Vector::new(0.5, 3.0)),
        position: [-2.0, 0.0],
        angle: 0.15,
    });
    let mut w = World::from_level(&l, Team::Blue).unwrap();
    let before = w
        .bodies()
        .filter(|b| b.material == Material::Anchor)
        .map(|b| (b.position, b.angle))
        .collect::<Vec<_>>();
    for ammo in Ammunition::ALL {
        for tick in 0..120 {
            w.step(&if tick % 60 == 0 {
                vec![Command::Fire(Shot::new(ammo, Vector::new(-2.0, 0.0)))]
            } else {
                vec![]
            });
        }
    }
    for _ in 0..480 {
        w.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
    }
    assert!(w.launcher_position().x < -3.0);
    assert_eq!(w.stats().broken, 0);
    assert_eq!(
        before,
        w.bodies()
            .filter(|b| b.material == Material::Anchor)
            .map(|b| (b.position, b.angle))
            .collect::<Vec<_>>()
    );
}
#[test]
fn authored_bridges_cross_and_keep_deck_immune_in_both_orientations() {
    for vertical in [false, true] {
        let mut l = fixture();
        l.items.retain(|i| !matches!(i, LevelItem::Flag { .. }));
        // This fixture tests the crossing alone; the old red spawn is inside
        // the vertical river and would now instantiate an invalid opponent.
        l.items.retain(|i| {
            !matches!(
                i,
                LevelItem::Spawn {
                    team: Team::Red,
                    ..
                }
            )
        });
        if vertical {
            l.size = [24.0, 24.0];
            l.items[0] = LevelItem::Spawn {
                team: Team::Blue,
                position: [0.0, -7.0],
                angle: std::f32::consts::FRAC_PI_2,
            };
        }
        let (min, max, water_min, water_max) = if vertical {
            ([-1.5, -3.4], [1.5, 3.4], [-11.5, -2.3], [11.5, 2.3])
        } else {
            ([-3.4, -1.5], [3.4, 1.5], [-2.3, -7.1], [2.3, 7.1])
        };
        l.items.push(LevelItem::Terrain {
            terrain: crate::TerrainKind::Water,
            min: water_min,
            max: water_max,
        });
        l.items.push(LevelItem::Bridge { min, max, vertical });
        let mut w = World::from_level(&l, Team::Blue).unwrap();
        for ammo in Ammunition::ALL {
            w.step(&[Command::Fire(Shot::new(ammo, Vector::ZERO))]);
            for _ in 0..60 {
                w.step(&[]);
            }
        }
        assert!(w.bridges().all(|b| b.intact));
        assert_eq!(w.stats().broken, 0);
        for _ in 0..600 {
            w.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
        }
        let p = w.launcher_position();
        assert!(
            if vertical { p.y > 5.0 } else { p.x > 5.0 },
            "vertical={vertical}: {p:?}"
        );
        assert!(w.bridges().all(|b| b.intact));
    }
}

#[test]
fn bridge_booths_need_real_banks() {
    let mut l = fixture();
    l.items.push(LevelItem::Terrain {
        terrain: crate::TerrainKind::Water,
        min: [-4.0, -6.0],
        max: [4.0, 6.0],
    });
    l.items.push(LevelItem::Bridge {
        min: [-3.4, -1.5],
        max: [3.4, 1.5],
        vertical: false,
    });
    assert!(
        World::from_level(&l, Team::Blue)
            .err()
            .unwrap()
            .contains("dry banks")
    );
}

#[test]
fn large_bridge_remains_intact_at_rest_and_only_booths_are_targetable() {
    for vertical in [false, true] {
        let rotate = |p: [f32; 2]| if vertical { [-p[1], p[0]] } else { p };
        let mut level = Level {
            size: [200., 200.],
            items: vec![LevelItem::Spawn {
                team: Team::Blue,
                position: rotate([-70., 0.]),
                angle: if vertical {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.
                },
            }],
            ..Level::default()
        };
        let (min, max) = if vertical {
            ([-10., -60.], [10., 60.])
        } else {
            ([-60., -10.], [60., 10.])
        };
        level.items.push(LevelItem::Bridge { min, max, vertical });
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        for _ in 0..240 {
            world.step(&[]);
        }
        assert!(world.bridges().all(|b| b.intact));
        assert_eq!(world.stats().broken, 0);
        let targetable = world
            .pieces
            .iter()
            .filter(|p| p.object.is_some() && !p.bridge_deck)
            .count();
        assert_eq!(
            targetable, 16,
            "Both complete booths retain targetable cells"
        );
        assert!(world.stats().cells < 300);
    }
}

#[test]
fn merged_rocks_form_one_rigid_body_without_duplicate_material() {
    let mut a = LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(LibraryPreset::Rock.asset()),
        position: [-1., 0.],
        angle: 0.,
    };
    let b = LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(LibraryPreset::Rock.asset()),
        position: [1., 0.],
        angle: 0.3,
    };
    a.resize(Vector::new(4., 4.)).unwrap();
    let merged = crate::level::merge_items(&[a, b]).unwrap();
    let mut level = fixture();
    level.items.push(merged);
    let world = World::from_level(&level, Team::Blue).unwrap();
    let bodies: std::collections::HashSet<_> = world
        .pieces
        .iter()
        .filter(|p| p.object.is_some())
        .map(|p| p.body)
        .collect();
    assert_eq!(bodies.len(), 1);
}

#[test]
fn playable_top_speed_is_higher_and_braking_still_stops_the_tank() {
    for (terrain, target) in [
        (crate::TerrainKind::Ground, 6.5),
        (crate::TerrainKind::Road, 6.5 * 1.45),
        (crate::TerrainKind::Sand, 6.5 * 0.48),
    ] {
        let level = Level {
            size: [200., 24.],
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-60., 0.],
                    angle: 0.,
                },
                LevelItem::Terrain {
                    terrain,
                    min: [-99., -10.],
                    max: [99., 10.],
                },
            ],
            ..Level::default()
        };
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        for _ in 0..1200 {
            w.step(&[Command::Tracks {
                left: 1.,
                right: 1.,
            }]);
        }
        let speed = w.physics.bodies[w.pieces[w.launcher].body].linvel().x;
        assert!(
            (speed - target).abs() < 0.15,
            "{terrain:?}: {speed} vs {target}"
        );
        for _ in 0..240 {
            w.step(&[]);
        }
        assert!(
            w.physics.bodies[w.pieces[w.launcher].body]
                .linvel()
                .length()
                < 0.05
        );
    }
}

#[test]
fn brittle_fence_breaks_under_a_lighter_load_than_wall() {
    let mut thresholds = Vec::new();
    for preset in [LibraryPreset::Wall, LibraryPreset::Fence] {
        let mut threshold = None;
        for force in [100.0, 300.0, 1000.0, 3000.0, 10000.0, 30000.0] {
            let mut world = World::from_object(&preset.asset()).unwrap();
            let loaded: Vec<_> = world
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.object == Some(0) && p.previous_position.y < -1.0)
                .map(|(i, _)| i)
                .collect();
            let event = world.next_impact();
            for &id in &loaded {
                world.loads.push(loads::Load {
                    piece: id,
                    point: world.piece_pose(id).translation,
                    impulse: Vector::new(force * TICK_DT / loaded.len() as f32, 0.0),
                    energy: 100.0 / loaded.len() as f32,
                    apply: true,
                    event,
                });
            }
            world.process_loads(TICK_DT, &mut Vec::new());
            assert_eq!(world.stats().plastic_work, 0.0);
            assert!(world.bending_joints.is_empty());
            if world.broken > 0 {
                threshold = Some(force);
                break;
            }
        }
        thresholds.push(threshold.unwrap_or(f32::INFINITY));
    }
    println!("first fracture load, wall / fence: {thresholds:?} N");
    assert!(
        thresholds[1] < thresholds[0],
        "fence must fracture under a lighter load"
    );
}
