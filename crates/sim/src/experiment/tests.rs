use super::*;
use crate::{Command, Material, Shape, Shot, Vec2, World};
#[test]
fn ammunition_bursts_and_laser_cuts_replay_with_saved_effects() {
    let mut object = crate::ObjectAsset::workshop_preset(crate::ObjectPreset::Plate);
    object.thickness = 0.02;
    let mut e = Experiment::new(object.clone());
    e.shared_impact = false;
    e.variants = Ammunition::ALL
        .into_iter()
        .map(|ammo| {
            let mut impact = Impact::default();
            impact.select_ammunition(ammo);
            Variant {
                name: ammo.label().into(),
                object: object.clone(),
                impact,
            }
        })
        .collect();
    e.duration_ticks = 240;
    let mut first = Runner::new(e.clone()).unwrap();
    while first.advance().is_some() {}
    assert_eq!(
        first
            .worlds
            .iter()
            .map(|w| w.stats().shots)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 20]
    );
    assert!(
        first.recordings[3]
            .frames
            .iter()
            .any(|f| f.beam.is_some() && f.metrics.ablated_area > 0.0)
    );
    e.cursor_tick = 120;
    e.review = first.recordings.iter().map(|r| r.state(120)).collect();
    let loaded = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(loaded.variants, e.variants);
    assert_eq!(loaded.review[3].frame, e.review[3].frame);
    let mut replay = Runner::new(loaded).unwrap();
    while replay.advance().is_some() {}
    for (a, b) in first.recordings.iter().zip(&replay.recordings) {
        assert!(a.frames == b.frames, "saved ammunition changed the replay");
        assert!(a.geometry == b.geometry);
    }
    let legacy = Experiment::new(object);
    let mut json = serde_json::to_value(&legacy).unwrap();
    for v in json["variants"].as_array_mut().unwrap() {
        v["impact"].as_object_mut().unwrap().remove("ammunition");
    }
    let loaded = Experiment::from_json(&json.to_string()).unwrap();
    assert!(
        loaded
            .variants
            .iter()
            .all(|v| v.impact.ammunition == Ammunition::HighExplosive)
    );
    let mut invalid = e.clone();
    invalid.variants[2].impact.shell_speed = 350.0;
    assert!(
        invalid.validate().is_err(),
        "Rocket speed must stay in its slow travel range"
    );
    invalid = e;
    invalid.review[3].frame.beam = Some(([f32::NAN, 0.0], [0.0; 2]));
    assert!(invalid.validate().is_err());
}

#[test]
fn ground_attachments_and_edge_supports_replay_from_saved_recipes() {
    let mut e = Experiment::new(crate::LibraryPreset::Rock.asset());
    e.variants.truncate(2);
    e.variants[1].object = crate::LibraryPreset::Wall.asset();
    e.variants[1].object.additional_pinned_edges = vec![0];
    e.duration_ticks = 240;
    let mut first = Runner::new(e.clone()).unwrap();
    while first.advance().is_some() {}
    e.cursor_tick = 120;
    e.review = first.recordings.iter().map(|r| r.state(120)).collect();
    let loaded = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(loaded.variants, e.variants);
    let mut replay = Runner::new(loaded).unwrap();
    while replay.advance().is_some() {}
    for (a, b) in first.recordings.iter().zip(&replay.recordings) {
        assert!(a.frames == b.frames, "saved attachments changed the replay");
        assert!(a.geometry == b.geometry);
    }
}

#[test]
fn tuned_barrel_leaves_backstop_and_preserves_the_free_shot_reference() {
    for backstop in [true, false] {
        let mut e = Experiment::barrel_shot_comparison();
        let mut v = e.variants[1].clone();
        v.impact.backstop = backstop;
        e.variants = vec![v.clone(), v];
        e.variants[1].object = crate::LibraryPreset::Barrel.asset();
        e.favorite = None;
        let mut runner = Runner::new(e.clone()).unwrap();
        while runner.advance().is_some() {}
        for w in &runner.worlds {
            assert_eq!(w.stats().broken, 0);
            assert_eq!(w.stats().unconverged, 0);
            assert_eq!(w.stats().shells, 0);
        }
        if backstop {
            let right = |w: &World| {
                w.bodies()
                    .filter(|b| b.material == Material::Panel)
                    .flat_map(|b| {
                        let Shape::Triangle { vertices } = b.shape else {
                            unreachable!()
                        };
                        let (sin, cos) = b.angle.sin_cos();
                        vertices.map(|p| b.position.x + cos * p.x - sin * p.y)
                    })
                    .fold(f32::NEG_INFINITY, f32::max)
            };
            assert!(3.8 - right(&runner.worlds[0]) < 0.02);
            let gap = 3.8 - right(&runner.worlds[1]);
            assert!(gap > 0.3 && gap < 1.0, "clear wall separation, gap={gap}");
        } else {
            // Includes the first hard arena-wall collision and all later frames.
            let mut position_error = 0.0_f32;
            let mut angle_error = 0.0_f32;
            for (a, b) in runner.recordings[0]
                .frames
                .iter()
                .zip(&runner.recordings[1].frames)
            {
                assert_eq!(a.tick, b.tick);
                assert_eq!(a.poses.len(), b.poses.len());
                for (p, q) in a.poses.iter().zip(&b.poses) {
                    assert_eq!(p.0, q.0);
                    position_error = position_error.max(Vec2::from(p.1).distance(Vec2::from(q.1)));
                    angle_error = angle_error.max((p.2 - q.2).abs());
                }
            }
            assert!(
                position_error < 0.001 && angle_error < 0.001,
                "free reference errors: position {position_error} m, angle {angle_error} rad"
            );
            assert!(runner.recordings[0].geometry == runner.recordings[1].geometry);
        }
        e.cursor_tick = 120;
        e.review = runner.recordings.iter().map(|r| r.state(120)).collect();
        let mut replay =
            Runner::new(Experiment::from_json(&e.to_json().unwrap()).unwrap()).unwrap();
        while replay.advance().is_some() {}
        for (a, b) in runner.recordings.iter().zip(&replay.recordings) {
            assert_eq!(a.frames, b.frames);
            assert_eq!(a.geometry, b.geometry);
        }
    }
}

#[test]
fn replay_includes_actual_arena_walls_without_counting_them_as_object_area() {
    let mut e = Experiment::barrel_free_shot_comparison();
    let mut runner = Runner::new(e.clone()).unwrap();
    for (i, r) in runner.recordings.iter().enumerate() {
        let state = r.state(0);
        let boundary_ids: Vec<_> = state
            .geometry
            .iter()
            .filter(|g| g.material == 5)
            .map(|g| g.id)
            .collect();
        let mut recorded: Vec<_> = state
            .frame
            .poses
            .iter()
            .filter(|p| boundary_ids.contains(&p.0))
            .map(|p| p.1)
            .collect();
        let mut actual: Vec<_> = runner.worlds[i]
            .bodies()
            .filter(|b| b.material == Material::Boundary)
            .map(|b| b.position.to_array())
            .collect();
        let by_x = |a: &[f32; 2], b: &[f32; 2]| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]));
        recorded.sort_by(by_x);
        actual.sort_by(by_x);
        assert_eq!(recorded, actual);
        assert_eq!(recorded.len(), 4);
        assert!(
            (state.frame.metrics.largest_area - e.variants[i].object.mesh().unwrap().area).abs()
                < 1e-5
        );
    }
    while runner.advance().is_some() {}
    e.cursor_tick = 120;
    e.review = runner.recordings.iter().map(|r| r.state(120)).collect();
    let saved = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    let mut replay = Runner::new(saved).unwrap();
    while replay.advance().is_some() {}
    for (a, b) in runner.recordings.iter().zip(&replay.recordings) {
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.geometry, b.geometry);
    }
}

#[test]
fn shot_contact_matrix_rebounds_slows_and_replays_after_serialization() {
    let mut e = Experiment::barrel_shot_comparison();
    let mut reference = crate::LibraryPreset::Barrel.asset();
    reference.soft_bounce = None;
    assert_eq!(e.variants.len(), 6);
    assert!(e.shared_impact);
    for v in &e.variants {
        let mut normalized = v.object.clone();
        normalized.friction = reference.friction;
        normalized.restitution = reference.restitution;
        assert_eq!(normalized, reference);
        assert_eq!(v.impact, e.variants[0].impact);
        assert_eq!(v.impact.kind, ImpactKind::Shell);
    }
    let mut first = Runner::new(e.clone()).unwrap();
    while first.advance().is_some() {}
    let positions: Vec<_> = first
        .worlds
        .iter()
        .map(|w| {
            assert_eq!(w.stats().shells, 0);
            assert_eq!(w.stats().explosions, 1);
            assert_eq!(w.stats().broken, 0);
            assert_eq!(w.stats().unconverged, 0);
            let bodies: Vec<_> = w
                .bodies()
                .filter(|b| b.material == Material::Panel)
                .collect();
            bodies.iter().map(|b| b.position.x).sum::<f32>() / bodies.len() as f32
        })
        .collect();
    // Larger restitution produces more rebound from the same fixture;
    // increasing floor friction reduces the rebound's travel distance.
    assert!(positions[3] < positions[0] - 1.0, "{positions:?}");
    assert!(positions[3] < positions[4] - 0.5, "{positions:?}");
    assert!(positions[4] < positions[5] - 0.1, "{positions:?}");
    e.cursor_tick = e.duration_ticks;
    e.favorite = Some(4);
    e.review = first
        .recordings
        .iter()
        .map(|r| r.state(e.cursor_tick))
        .collect();
    let saved = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(saved.variants, e.variants);
    assert_eq!(saved.favorite, e.favorite);
    let mut replay = Runner::new(saved).unwrap();
    while replay.advance().is_some() {}
    for (a, b) in first.recordings.iter().zip(&replay.recordings) {
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.geometry, b.geometry);
    }
}

#[test]
fn six_deformation_limits_share_the_test_and_replay_saved_cases() {
    let mut e = Experiment::crush_comparison();
    assert_eq!(e.variants.len(), 6);
    let mut first = Runner::new(e.clone()).unwrap();
    for (i, (v, w)) in e.variants.iter().zip(&first.worlds).enumerate() {
        let mut normalized = v.object.clone();
        normalized
            .components
            .plasticity
            .as_mut()
            .unwrap()
            .max_strain = 0.4;
        assert_eq!(normalized, e.variants[0].object);
        assert_eq!(v.impact, e.variants[0].impact);
        assert_eq!(
            v.object.components.plasticity.unwrap().max_strain,
            [0.4, 0.8, 1.2, 1.6, 2.2, 3.0][i]
        );
        assert!(v.impact.backstop);
        assert_eq!(v.impact.tank_speed, 0.8);
        assert_eq!(v.impact.aim[1], 0.0);
        let tank = w
            .bodies()
            .find(|b| b.material == Material::Launcher)
            .unwrap();
        assert_eq!(tank.position.y, v.impact.aim[1]);
        assert!(
            (w.stats().kinetic_energy - 0.5 * v.impact.tank_mass * v.impact.tank_speed.powi(2))
                .abs()
                < 0.1
        );
    }
    let initial = first.recordings[0].state(0);
    while first.advance().is_some() {}
    assert_eq!(first.completed_tick, 960);
    assert!(!first.memory_limited);
    for r in &first.recordings {
        assert_eq!(r.frames.last().unwrap().tick, 960);
        assert!(
            r.frames
                .iter()
                .all(|f| f.metrics.unconverged == 0 && f.metrics.broken == 0)
        );
        assert!(r.frames.last().unwrap().metrics.plastic_work > 0.0);
    }
    assert_eq!(first.recordings[0].state(0).geometry, initial.geometry);
    // Different material limits must produce different recorded shapes.
    let object_poses = |i: usize| {
        let state = first.recordings[i].state(240);
        state
            .frame
            .poses
            .into_iter()
            .filter(|p| {
                state
                    .geometry
                    .iter()
                    .any(|g| g.id == p.0 && g.material == 2)
            })
            .collect::<Vec<_>>()
    };
    assert_ne!(object_poses(0), object_poses(1));
    assert_ne!(object_poses(1), object_poses(2));
    assert_ne!(object_poses(0), object_poses(3));
    e.favorite = Some(4);
    e.cursor_tick = 720;
    e.review = first.recordings.iter().map(|r| r.state(720)).collect();
    let restored = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(restored.variants, e.variants);
    assert_eq!(restored.favorite, Some(4));
    assert!(restored.review.iter().all(|r| r.frame.tick == 720));
    let mut replay = Runner::new(restored).unwrap();
    while replay.advance().is_some() {}
    for (a, b) in first.recordings.iter().zip(&replay.recordings) {
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.geometry, b.geometry);
    }
}

#[test]
fn angled_shared_trials_set_pose_and_velocity_and_preserve_old_recipes() {
    for (angle, offset, backstop) in [
        (-30.0_f32, -0.82, true),
        (30.0, 0.82, true),
        (90.0, 0.0, false),
        (180.0, 0.0, false),
    ] {
        let mut e = Experiment::crush_comparison();
        e.duration_ticks = 240;
        for v in &mut e.variants {
            v.impact.angle_degrees = angle;
            v.impact.aim[1] = offset;
            v.impact.tank_speed = 2.0;
            v.impact.backstop = backstop;
        }
        let mut first = Runner::new(e.clone()).unwrap();
        let a = angle.to_radians();
        let forward = Vec2::new(a.cos(), a.sin());
        let side = Vec2::new(-forward.y, forward.x);
        for w in &first.worlds {
            let tank = w
                .bodies()
                .find(|b| b.material == Material::Launcher)
                .unwrap();
            assert!(tank.velocity.distance(forward * 2.0) < 1e-5);
            assert!((Vec2::new(tank.angle.cos(), tank.angle.sin()) - forward).length() < 1e-5);
            assert!(((tank.position - Vec2::new(3.0, 0.0)).dot(side) - offset).abs() < 1e-5);
        }
        while first.advance().is_some() {}
        e.cursor_tick = 240;
        e.favorite = Some(2);
        e.review = first.recordings.iter().map(|r| r.state(240)).collect();
        let saved = Experiment::from_json(&e.to_json().unwrap()).unwrap();
        assert!(saved.shared_impact);
        assert_eq!(saved.variants, e.variants);
        let mut replay = Runner::new(saved).unwrap();
        while replay.advance().is_some() {}
        for (a, b) in first.recordings.iter().zip(&replay.recordings) {
            assert_eq!(a.frames, b.frames);
            assert_eq!(a.geometry, b.geometry);
        }
    }
    let e = Experiment::crush_comparison();
    let mut old = serde_json::to_value(&e).unwrap();
    old.as_object_mut().unwrap().remove("shared_impact");
    for v in old["variants"].as_array_mut().unwrap() {
        v["impact"].as_object_mut().unwrap().remove("angle_degrees");
    }
    let old = Experiment::from_json(&old.to_string()).unwrap();
    assert!(!old.shared_impact);
    assert_eq!(old.variants, e.variants);
    let mut invalid = e.clone();
    invalid.variants[0].impact.tank_speed = 3.0;
    assert!(invalid.validate().is_err());
    for v in &mut invalid.variants {
        v.impact = e.variants[0].impact.clone();
        v.impact.angle_degrees = 181.0;
    }
    assert!(invalid.validate().is_err());
    for v in &mut invalid.variants {
        v.impact.angle_degrees = 180.0;
    }
    assert!(
        Runner::new(invalid).is_err(),
        "the opposite approach starts in the fixed backstop and must be rejected"
    );
}

#[test]
fn ten_barrels_share_a_fixture_and_replay_exactly_with_a_saved_favorite() {
    let mut e = Experiment::barrel_comparison();
    assert_eq!(e.variants.len(), 10);
    assert!(e.variants.iter().all(|v| v.impact == e.variants[0].impact
        && v.object.outline == e.variants[0].object.outline
        && v.object.density == 140.0
        && v.impact.backstop));
    let mut first = Runner::new(e.clone()).unwrap();
    let initial = first.recordings[0].state(0);
    while first.advance().is_some() {}
    assert_eq!(first.completed_tick, e.duration_ticks);
    assert_eq!(
        first.recordings[0].state(0).geometry,
        initial.geometry,
        "dents must not rewrite earlier geometry"
    );
    assert_eq!(first.recordings[0].state(0).frame, initial.frame);
    assert_ne!(first.recordings[0].state(480).geometry, initial.geometry);
    let counts: std::collections::BTreeSet<_> = first
        .recordings
        .iter()
        .map(|r| {
            assert_eq!(r.frames.last().unwrap().tick, e.duration_ticks);
            assert!(r.frames.iter().all(|f| f.metrics.unconverged == 0));
            assert!(r.frames.iter().all(|f| f.metrics.broken == 0));
            for f in &r.frames {
                let outline: u32 = f
                    .poses
                    .iter()
                    .filter(|p| r.geometry[p.0].as_ref().unwrap().material == 2)
                    .map(|p| p.3.count_ones())
                    .sum();
                assert_eq!(
                    outline, 16,
                    "plastic deformation must not expose internal construction edges"
                );
            }
            (r.frames.last().unwrap().metrics.plastic_strain * 100.0) as u32
        })
        .collect();
    assert!(
        counts.len() >= 5,
        "the study should span visibly different permanent dents"
    );
    e.favorite = Some(9);
    e.cursor_tick = 180;
    e.review = first.recordings.iter().map(|r| r.state(180)).collect();
    let restored = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(restored.favorite, Some(9));
    assert_eq!(restored.review.len(), 10);
    let mut second = Runner::new(restored).unwrap();
    while second.advance().is_some() {}
    for (a, b) in first.recordings.iter().zip(&second.recordings) {
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.geometry, b.geometry);
    }
    e.review.clear();
    e.variants.push(e.variants[0].clone());
    assert!(e.validate().is_err());
}
#[test]
fn explosive_components_and_colors_survive_saved_replay() {
    let asset = crate::LibraryPreset::ExplosiveBarrel.asset();
    let mut e = Experiment::new(asset.clone());
    e.duration_ticks = 120;
    e.variants = vec![e.variants[1].clone()]; // The unscaled reference.
    let mut first = Runner::new(e.clone()).unwrap();
    assert!(
        first.recordings[0]
            .geometry
            .iter()
            .flatten()
            .any(|g| g.color == asset.color)
    );
    while first.advance().is_some() {}
    assert_eq!(first.worlds[0].stats().explosions, 2);
    e.review = first.recordings.iter().map(|r| r.state(120)).collect();
    e.cursor_tick = 120;
    let restored = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(restored.variants[0].object.components, asset.components);
    assert_eq!(restored.review[0].geometry, e.review[0].geometry);
    let mut second = Runner::new(restored).unwrap();
    while second.advance().is_some() {}
    assert_eq!(first.recordings[0].frames, second.recordings[0].frames);
}
#[test]
fn saved_recipe_replays_the_same_frames_and_preserves_a_review_checkpoint() {
    let mut e = Experiment::new(ObjectAsset::workshop_preset(crate::ObjectPreset::Wall));
    e.duration_ticks = 120;
    let mut a = Runner::new(e.clone()).unwrap();
    while a.advance().is_some() {}
    e.cursor_tick = 80;
    e.favorite = Some(2);
    e.review = a.recordings.iter().map(|r| r.state(80)).collect();
    let restored = Experiment::from_json(&e.to_json().unwrap()).unwrap();
    assert_eq!(restored.favorite, Some(2));
    for (x, y) in restored.review.iter().zip(&e.review) {
        assert_eq!(x.frame, y.frame);
        assert_eq!(x.geometry, y.geometry);
    }
    let mut b = Runner::new(restored).unwrap();
    while b.advance().is_some() {}
    for (x, y) in a.recordings.iter().zip(&b.recordings) {
        assert_eq!(x.frames, y.frames);
    }
}
#[test]
fn tank_trials_change_actual_vehicle_mass_and_break_the_target() {
    let mut e = Experiment::new(ObjectAsset::workshop_preset(crate::ObjectPreset::Wall));
    e.duration_ticks = 240;
    e.variants.truncate(1);
    e.variants[0].impact.kind = ImpactKind::Tank;
    e.variants[0].impact.tank_mass = 9000.0;
    e.variants[0].impact.tank_speed = 6.0;
    let mut r = Runner::new(e).unwrap();
    assert!((r.worlds[0].stats().kinetic_energy - 0.5 * 9000.0 * 6.0 * 6.0).abs() < 1.0);
    while r.advance().is_some() {}
    assert!(r.recordings[0].frames.last().unwrap().metrics.broken > 0);
    assert_eq!(
        r.recordings[0].frames.last().unwrap().metrics.unconverged,
        0
    );
}
#[test]
fn generated_grains_remain_rigid_and_stress_solutions_converge_on_impact() {
    for preset in crate::ObjectPreset::ALL {
        for mode in [crate::GrainMode::Stones, crate::GrainMode::Bricks] {
            let mut object = ObjectAsset::workshop_preset(preset);
            object.regenerate_grains(mode, [0.7, 0.5], 7);
            let mut world = World::from_object(&object).unwrap();
            let original: Vec<_> = world.bodies().map(|b| (b.position, b.angle)).collect();
            for _ in 0..120 {
                world.step(&[]);
            }
            assert_eq!(world.stats().broken, 0, "{} {mode:?}", object.name);
            for (b, (p, a)) in world.bodies().zip(original) {
                assert!(b.position.distance(p) < 1e-4 && (b.angle - a).abs() < 1e-4);
            }
            assert!(
                world.bodies().any(|b| b.edges != 7),
                "grain interior edges should be hidden"
            );
            world.step(&[Command::Fire(Shot {
                ammunition: crate::Ammunition::HighExplosive,
                target: Vec2::new(3.0, 0.0),
                mass: 6.0,
                speed: 220.0,
            })]);
            for _ in 0..120 {
                world.step(&[]);
            }
            assert_eq!(world.stats().unconverged, 0, "{} {mode:?}", object.name);
            assert!(
                world
                    .bodies()
                    .all(|b| b.position.is_finite() && b.angle.is_finite())
            );
        }
    }
}
#[test]
fn malformed_experiments_and_unbounded_values_are_rejected() {
    let mut e = Experiment::new(ObjectAsset::preset(crate::ObjectPreset::Rock));
    e.variants[0].impact.shell_mass = f32::NAN;
    assert!(e.validate().is_err());
    e.variants[0].impact.shell_mass = 6.0;
    e.favorite = Some(50);
    assert!(e.validate().is_err());
    assert!(Experiment::from_json("{}").is_err());
    let mut e = Experiment::new(ObjectAsset::preset(crate::ObjectPreset::Rock));
    e.variants[0].object.young = Some(-1.0);
    assert!(Runner::new(e).is_err());
}
