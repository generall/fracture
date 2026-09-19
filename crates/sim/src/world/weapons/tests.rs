use super::*;

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
