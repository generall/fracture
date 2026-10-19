use super::*;

#[test]
fn live_spatial_sweeps_match_full_scans_after_geometry_and_velocity_changes() {
    use crate::level::{Level, Team};
    let mut w = World::from_level(&Level::default(), Team::Blue).unwrap();
    w.enemy = None;
    let moving = w.add_box(
        Vector::new(-5.0, 3.0),
        Vector::splat(0.25),
        Material::Rock,
        false,
    );
    w.physics.bodies[w.pieces[moving].body].set_linvel(Vector::new(0.0, -400.0), true);
    // Exercise repeated queries within one physics tick, where fixed geometry
    // is reused but moving targets must remain current after every pose edit.
    w.in_tick = true;
    w.query_epoch += 1;
    let check = |w: &World| {
        for i in 0..128 {
            let origin = Vector::new(
                -18.0 + (i % 3) as f32 * 6.0,
                ((i / 3) % 5) as f32 * 2.0 - 5.0,
            );
            let velocity = Vector::from_angle(i as f32 * 2.399963) * 220.0;
            let dt = [0.002, 0.02, 0.1][i % 3];
            let radius = [0.035, 0.12][i % 2];
            let expected =
                w.cast_shell_targets(w.shell_targets(), origin, velocity, dt, radius, w.launcher);
            assert_eq!(
                w.shell_hit(origin, velocity, dt, radius, w.launcher),
                expected,
                "query {i}"
            );
        }
    };
    check(&w);
    let origin = Vector::new(-5.5, 0.0);
    let expected = w.cast_shell_targets(
        w.shell_targets(),
        origin,
        Vector::X * 100.0,
        0.0125,
        0.035,
        w.launcher,
    );
    assert!(
        expected.is_some(),
        "a moving target enters the otherwise empty sweep"
    );
    assert_eq!(
        w.shell_hit(origin, Vector::X * 100.0, 0.0125, 0.035, w.launcher),
        expected
    );
    w.physics.colliders[w.pieces[moving].collider].set_shape(SharedShape::cuboid(2.0, 0.15));
    w.physics.bodies[w.pieces[moving].body]
        .set_position(Pose::new(Vector::new(0.0, -2.0), 0.3), true);
    w.physics
        .bodies
        .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
    check(&w);
    let enemy = w.tanks[1].id;
    w.destroy_tank(enemy, &mut vec![]);
    check(&w);
    w.pieces[moving].active = false;
    w.physics.remove_body(w.pieces[moving].body);
    w.add_box(
        Vector::new(3.0, 1.0),
        Vector::splat(0.5),
        Material::Rock,
        false,
    );
    check(&w);
}

#[test]
fn shared_projectile_targets_refresh_after_death_and_fracture() {
    use crate::level::{Level, Team};
    for rocket in [false, true] {
        let fixture = || {
            let mut w = if rocket {
                World::from_objects(&[(crate::LibraryPreset::Rock.asset(), Vector::ZERO)]).unwrap()
            } else {
                World::from_level(&Level::default(), Team::Blue).unwrap()
            };
            w.enemy = None;
            if !rocket {
                let enemy = w.tanks[1].id;
                w.tanks[1].health = 12.0;
                w.physics.bodies[w.pieces[enemy].body]
                    .set_position(Pose::new(Vector::ZERO, 0.0), true);
                w.physics
                    .bodies
                    .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
            }
            w
        };
        let mut batch = fixture();
        batch.in_tick = true;
        batch.query_epoch += 1;
        let mut reference = fixture();
        let shells: Vec<_> = [0.0, 0.12, -0.12]
            .into_iter()
            .map(|y| Shell {
                vehicle: if rocket {
                    VehicleKind::Tank
                } else {
                    VehicleKind::Scout
                },
                flight: 0.0,
                ammunition: if rocket {
                    Ammunition::Rocket
                } else {
                    Ammunition::ArmorPiercing
                },
                position: Vector::new(-5.0, y),
                previous_position: Vector::new(-5.0, y),
                velocity: Vector::X * if rocket { 12.0 } else { 260.0 },
                mass: if rocket { 16.0 } else { 0.02 },
                age: 0.0,
                owner: batch.launcher,
            })
            .collect();
        batch.shells = shells.clone();
        let mut batch_events = vec![];
        let mut reference_events = vec![];
        // One shared pass must match individual fresh queries even when the
        // first impact creates wreck colliders or replaces shattered geometry.
        batch.advance_shells(if rocket { 0.5 } else { 0.025 }, &mut batch_events);
        let mut survivors = vec![];
        for shell in shells {
            reference.shells.push(shell);
            reference.advance_shells(if rocket { 0.5 } else { 0.025 }, &mut reference_events);
            survivors.append(&mut reference.shells);
        }
        reference.shells = survivors;
        assert_eq!(format!("{batch_events:?}"), format!("{reference_events:?}"));
        assert_eq!(
            format!("{:?}", batch.stats()),
            format!("{:?}", reference.stats())
        );
        assert_eq!(batch.shells.len(), reference.shells.len());
        assert!(batch_events.iter().any(|event| if rocket {
            matches!(event, Event::Exploded { .. })
        } else {
            matches!(event, Event::TankDestroyed { .. })
        }));
    }
}

#[test]
fn blast_resultant_does_not_depend_on_how_the_surface_is_tessellated() {
    let mut reference: Option<[f64; 4]> = None;
    for detail in 0..=4 {
        let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
        asset.refinement = detail;
        let mut world = World::from_object(&asset).unwrap();
        world.detonate(Vector::new(1.1, 0.35), 6.0, &mut Vec::new());
        let mut total = [0.0; 4];
        for pulse in &world.blast_pulses {
            let impulse = pulse.direction * pulse.impulse_left;
            let point = world
                .piece_pose(pulse.piece)
                .transform_point(pulse.local_point);
            total[0] += impulse.x as f64;
            total[1] += impulse.y as f64;
            total[2] += point.perp_dot(impulse) as f64;
            total[3] += pulse.energy_left as f64;
        }
        if let Some(expected) = reference {
            for i in 0..4 {
                // Components can nearly cancel; use the full resultant's
                // scale when allowing f32 collision-query roundoff.
                let scale = if i < 3 {
                    expected[0]
                        .abs()
                        .max(expected[1].abs())
                        .max(expected[2].abs())
                } else {
                    expected[3]
                };
                assert!(
                    (expected[i] - total[i]).abs() < scale.max(1.0) * 2e-5,
                    "detail {detail}: resultant {total:?}, expected {expected:?}"
                );
            }
        } else {
            reference = Some(total);
        }
    }
}

#[test]
fn blast_has_a_bounded_energy_budget_for_moving_bodies() {
    let mut world = World::new(Scene::LooseBlocks);
    for (x, velocity) in [(-4.5, 15.0), (-3.5, -20.0)] {
        world.add_ball(
            Vector::new(x, -5.0),
            0.1,
            20.0,
            Material::Masonry,
            Vector::new(velocity, 0.0),
        );
    }
    let before = world.stats().kinetic_energy;
    world.detonate(Vector::new(-4.0, -5.0), 6.0, &mut Vec::new());
    for _ in 0..24 {
        world.advance_blasts(TICK_DT / 4.0);
        world.process_loads(TICK_DT / 4.0, &mut Vec::new());
    }
    let added = world.stats().kinetic_energy - before;
    // A blast opposing motion can remove kinetic energy. Positive work
    // remains bounded regardless of the sign of the initial velocity.
    assert!(added.is_finite() && added < 45_000.0, "added {added} J");
}

#[test]
fn a_solid_wall_shields_a_body_from_blast_pressure() {
    fn receiver_speed(shielded: bool) -> f32 {
        let mut world = World::new(Scene::LooseBlocks);
        let id = world.add_ball(
            Vector::new(-3.0, -5.0),
            0.2,
            100.0,
            Material::Rock,
            Vector::ZERO,
        );
        if shielded {
            world.add_box(
                Vector::new(-3.5, -5.0),
                Vector::new(0.03, 0.5),
                Material::Anchor,
                true,
            );
        }
        world.detonate(Vector::new(-4.0, -5.0), 6.0, &mut Vec::new());
        for _ in 0..24 {
            world.advance_blasts(TICK_DT / 4.0);
            world.process_loads(TICK_DT / 4.0, &mut Vec::new());
        }
        world.physics.bodies[world.pieces[id].body]
            .linvel()
            .length()
    }
    assert!(receiver_speed(false) > 0.01);
    assert_eq!(receiver_speed(true), 0.0);
}

#[test]
fn adding_an_unrelated_receiver_does_not_redistribute_blast_energy() {
    fn velocity(extra_receiver: bool) -> Vector {
        let mut world = World::new(Scene::LooseBlocks);
        let id = world.add_ball(
            Vector::new(-3.0, -5.0),
            0.2,
            100.0,
            Material::Rock,
            Vector::ZERO,
        );
        if extra_receiver {
            world.add_ball(
                Vector::new(-5.0, -5.0),
                0.2,
                100.0,
                Material::Rock,
                Vector::ZERO,
            );
        }
        world.detonate(Vector::new(-4.0, -5.0), 6.0, &mut Vec::new());
        for _ in 0..24 {
            world.advance_blasts(TICK_DT / 4.0);
            world.process_loads(TICK_DT / 4.0, &mut Vec::new());
        }
        world.physics.bodies[world.pieces[id].body].linvel()
    }
    assert_eq!(velocity(false), velocity(true));
}

#[test]
fn eccentric_impact_on_light_debris_cannot_create_energy() {
    let mut world = World::new(Scene::LooseBlocks);
    let position = Vector::new(-4.0, -5.0);
    let id = world.add_ball(position, 0.1, 1.0, Material::Masonry, Vector::ZERO);
    let body = &mut world.physics.bodies[world.pieces[id].body];
    let shell = Shell {
        vehicle: VehicleKind::Tank,
        flight: 0.0,
        ammunition: Ammunition::HighExplosive,
        position,
        previous_position: position,
        velocity: Vector::new(400.0, 0.0),
        mass: 20.0,
        age: 0.0,
        owner: 0,
    };
    let before = body.kinetic_energy();
    transfer_impact(body, &shell, position + Vector::new(0.0, 0.1));
    let added = body.kinetic_energy() - before;
    assert!(added <= 1_600_001.0 && added > 0.0, "added {added} J");
}

#[test]
fn mortar_delivers_three_quarters_of_rocket_material_shock_energy() {
    let energies = [1.0, MORTAR_WARHEAD_SCALE].map(|scale| {
        let mut w = World::from_object(&crate::LibraryPreset::Rock.asset()).unwrap();
        let hit = w
            .pieces
            .iter()
            .position(|p| p.active && p.destructible())
            .unwrap();
        let event = w.next_impact();
        w.warhead_shock(hit, Vector::new(3.0, 0.0), scale, event, &mut vec![]);
        assert!(w.broken > 0, "internal shock must fracture real rock seams");
        w.fracture_work + w.loads.iter().map(|load| load.energy as f64).sum::<f64>()
    });
    assert!(energies[0] > 1_000_000.0);
    assert!(
        (energies[1] / energies[0] - 0.75).abs() < 1e-5,
        "{energies:?}"
    );
}
