use super::*;
fn shapes(w: &World) -> Vec<[Vector; 3]> {
    w.pieces
        .iter()
        .filter(|p| p.active && p.object == Some(0))
        .map(|p| {
            let Shape::Triangle { vertices } = p.shape else {
                panic!()
            };
            vertices.map(|v| p.local_pose.transform_point(v))
        })
        .collect()
}
fn span(w: &World, axis: Vector) -> f32 {
    let values: Vec<_> = w
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.active && p.object == Some(0))
        .flat_map(|(id, p)| {
            let Shape::Triangle { vertices } = p.shape else {
                panic!()
            };
            vertices.map(|v| w.piece_pose(id).transform_point(v).dot(axis))
        })
        .collect();
    values.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        - values.iter().copied().fold(f32::INFINITY, f32::min)
}
fn assert_rim_retained(w: &World) {
    for p in w.pieces.iter().filter(|p| p.active && p.object == Some(0)) {
        let Shape::Triangle { vertices } = p.shape else {
            panic!()
        };
        for i in 0..3 {
            if p.edges & (1 << i) != 0 {
                let length = vertices[i].distance(vertices[(i + 1) % 3]);
                assert!(
                    (length - p.reference_edges[i]).abs() < p.reference_edges[i] * 5e-5,
                    "rim contracted or stretched: {} -> {length}",
                    p.reference_edges[i]
                );
            }
        }
    }
}

#[test]
fn diametric_crushing_expands_the_transverse_span_without_shortening_the_frame() {
    for detail in [0, 1] {
        for angle in [
            0.0_f32,
            std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
        ] {
            let mut a = crate::LibraryPreset::Barrel.asset();
            a.refinement = detail;
            let mut w = World::from_object(&a).unwrap();
            let direction = Vector::new(angle.cos(), angle.sin());
            let transverse = perpendicular(direction);
            let axial_before = span(&w, direction);
            let transverse_before = span(&w, transverse);
            for _ in 0..100 {
                let event = w.next_impact();
                for sign in [-1.0, 1.0] {
                    let id = w
                        .pieces
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p.active && p.object == Some(0))
                        .max_by(|(a, _), (b, _)| {
                            (sign * w.piece_pose(*a).translation.dot(direction))
                                .total_cmp(&(sign * w.piece_pose(*b).translation.dot(direction)))
                        })
                        .unwrap()
                        .0;
                    w.loads.push(loads::Load {
                        piece: id,
                        point: w.piece_pose(id).translation,
                        impulse: direction * (-sign * 500.0),
                        energy: 100.0,
                        apply: false,
                        event,
                    });
                }
                w.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
                assert_rim_retained(&w);
            }
            let axial = span(&w, direction);
            let lateral = span(&w, transverse);
            assert!(axial < axial_before - 0.03, "frame must still crush");
            assert!(
                lateral > transverse_before + 0.02,
                "compression must bulge across the loading axis"
            );
            assert_eq!(w.stats().broken, 0);
            assert_eq!(w.stats().unconverged, 0);
        }
    }
}
#[test]
fn barrel_yields_under_pinning_and_retains_its_shape_after_unloading() {
    let a = crate::LibraryPreset::Barrel.asset();
    let mut w = World::from_object(&a).unwrap();
    w.add_test_backstop(&a);
    w.configure_test_tank(4500.0, Vector::new(0.87, 0.0), 0.8)
        .unwrap();
    let original = shapes(&w);
    let mass: f32 = w
        .pieces
        .iter()
        .filter(|p| p.object == Some(0))
        .map(Piece::mass)
        .sum();
    let mut buckled = false;
    for _ in 0..480 {
        buckled |= w
            .step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }])
            .iter()
            .any(|e| matches!(e,Event::Buckled{work,..} if *work>0.5));
    }
    assert!(buckled, "real barrel crushing reports a buckling event");
    assert!(w.stats().plastic_work > 0.0);
    assert!(w.stats().plastic_strain > 0.03);
    assert_eq!(w.stats().broken, 0);
    assert_ne!(shapes(&w), original);
    assert!(span(&w, Vector::X) < 1.25);
    assert!(
        span(&w, Vector::Y) > 1.35,
        "tank crushing must widen the barrel transversely"
    );
    assert_rim_retained(&w);
    let after = shapes(&w);
    // Remove the tool to isolate unloading from another reverse collision.
    w.configure_test_tank(4500.0, Vector::new(-7.0, 0.0), 0.0)
        .unwrap();
    for _ in 0..240 {
        w.step(&[]);
    }
    assert_eq!(
        shapes(&w),
        after,
        "unloaded geometry must never spring back"
    );
    let remaining: f32 = w
        .pieces
        .iter()
        .filter(|p| p.active && p.object == Some(0))
        .map(Piece::mass)
        .sum();
    assert!((mass - remaining).abs() < 1e-4);
    assert_eq!(w.stats().unconverged, 0);
}

fn contact_load(w: &mut World, impulse: Vector, energy: f32) {
    let (id, _) = w
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.active && p.object == Some(0))
        .min_by(|(a, _), (b, _)| {
            w.piece_pose(*a)
                .translation
                .x
                .total_cmp(&w.piece_pose(*b).translation.x)
        })
        .unwrap();
    let event = w.next_impact();
    w.loads.push(loads::Load {
        piece: id,
        point: w.piece_pose(id).translation,
        impulse,
        energy,
        apply: false,
        event,
    });
    w.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
}

#[test]
fn yielding_requires_both_stress_and_work_and_respects_the_strain_limit() {
    let mut a = crate::LibraryPreset::Barrel.asset();
    a.components.crush = None;
    a.components.plasticity.as_mut().unwrap().max_strain = 0.02;
    let mut w = World::from_object(&a).unwrap();
    let before = shapes(&w);
    contact_load(&mut w, Vector::new(1000.0, 0.0), 0.0);
    assert_eq!(
        shapes(&w),
        before,
        "stress alone cannot pay for deformation"
    );
    contact_load(&mut w, Vector::new(1e-7, 0.0), 100.0);
    assert_eq!(
        shapes(&w),
        before,
        "work alone cannot cross the yield threshold"
    );
    for _ in 0..100 {
        contact_load(&mut w, Vector::new(1000.0, 0.0), 100.0);
    }
    assert!(w.stats().plastic_strain > 0.019);
    assert!(w.stats().plastic_strain <= 0.020001);
    assert_eq!(w.stats().broken, 0);
    assert_eq!(w.stats().refinements, 0);
}

#[test]
fn shape_change_preserves_mass_and_momentum_and_cannot_spend_unavailable_work() {
    let mut a = crate::LibraryPreset::Barrel.asset();
    a.components.crush = None;
    let mut w = World::from_object(&a).unwrap();
    let handle = w.pieces.iter().find(|p| p.object == Some(0)).unwrap().body;
    w.physics.bodies[handle].set_linvel(Vector::new(2.0, -1.0), true);
    w.physics.bodies[handle].set_angvel(3.0, true);
    let totals = |w: &World| {
        let b = &w.physics.bodies[handle];
        let m = b.mass() as f64;
        let v = b.linvel();
        let c = b.center_of_mass();
        [
            m,
            m * v.x as f64,
            m * v.y as f64,
            m * c.perp_dot(v) as f64
                + b.angvel() as f64 / b.mass_properties().effective_world_inv_inertia as f64,
            b.kinetic_energy() as f64,
        ]
    };
    let before = totals(&w);
    let shape = shapes(&w);
    contact_load(&mut w, Vector::new(1000.0, 100.0), 0.1);
    assert_ne!(shapes(&w), shape);
    let after = totals(&w);
    for i in 0..4 {
        assert!(
            (before[i] - after[i]).abs() < before[i].abs().max(1.0) * 3e-5,
            "{before:?} -> {after:?}"
        );
    }
    assert!(w.plastic_work > 0.0 && w.plastic_work <= 0.100001);
    assert!(after[4] + w.plastic_work <= before[4] + 0.1001);
    // Every collision triangle follows the material, with reference mass.
    for p in w.pieces.iter().filter(|p| p.object == Some(0)) {
        let collider = &w.physics.colliders[p.collider];
        assert!((collider.mass() - p.mass()).abs() < 1e-5);
        let Shape::Triangle { vertices } = p.shape else {
            panic!()
        };
        let convex = collider.shape().as_convex_polygon().unwrap();
        assert!(vertices.iter().all(|v| {
            convex
                .points()
                .iter()
                .any(|p| p.distance_squared(*v) < 1e-10)
        }));
    }
}

#[test]
fn legacy_welded_fence_bends_without_detaching_its_support() {
    // Saved welded-frame fences retain their original material behavior.
    let mut a = crate::LibraryPreset::MetalFence.asset();
    a.density = 7800.0;
    a.friction = 0.6;
    a.regenerate_grains(crate::GrainMode::Bricks, [0.2, 0.3], 7);
    a.components.bending_torque = None;
    a.components.plasticity = Some(crate::Plasticity {
        yield_stress: 30_000.0,
        flow_rate: 1.0,
        hardening: 2.0,
        max_strain: 1.0,
    });
    let mut w = World::from_object(&a).unwrap();
    let before = shapes(&w);
    let bottom = before
        .iter()
        .flatten()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);
    w.configure_test_tank(4500.0, Vector::new(1.42, 0.0), 0.8)
        .unwrap();
    for _ in 0..480 {
        w.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
    }
    let after = shapes(&w);
    assert!(w.stats().plastic_strain > 0.01);
    assert_eq!(w.stats().broken, 0);
    assert_eq!(w.stats().chunks, 1);
    assert_eq!(w.stats().unconverged, 0);
    assert!(
        before
            .iter()
            .flatten()
            .zip(after.iter().flatten())
            .any(|(a, b)| a.distance(*b) > 0.05)
    );
    for (a, b) in before
        .iter()
        .flatten()
        .zip(after.iter().flatten())
        .filter(|(a, _)| (a.y - bottom).abs() < 1e-5)
    {
        assert!(a.distance(*b) < 1e-5, "support moved: {a:?} -> {b:?}");
    }
    for (i, &a) in after.iter().enumerate() {
        for &b in &after[i + 1..] {
            assert!(!overlaps(a, b));
        }
    }
    w.configure_test_tank(4500.0, Vector::new(-7.0, 0.0), 0.0)
        .unwrap();
    for _ in 0..120 {
        w.step(&[]);
    }
    assert_eq!(shapes(&w), after);
}

#[test]
#[ignore = "manual simulation timing benchmark"]
fn barrel_pinned_against_rubble_benchmark() {
    use std::hash::Hasher;
    let mut world = World::new(Scene::BarrelRubble);
    let mut elapsed = std::time::Duration::ZERO;
    let mut worst = std::time::Duration::ZERO;
    let mut digest = std::collections::hash_map::DefaultHasher::new();
    for _ in 0..480 {
        let start = std::time::Instant::now();
        let events = world.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
        let dt = start.elapsed();
        elapsed += dt;
        worst = worst.max(dt);
        digest.write(
            format!(
                "{events:?}{:?}{:?}",
                world.stats(),
                world.bodies().collect::<Vec<_>>()
            )
            .as_bytes(),
        );
    }
    assert!(
        world.stats().plastic_work > 0.0,
        "driving must crush the barrel"
    );
    println!(
        "barrel/rubble: {:.3} ms/tick, worst {:.3} ms, fingerprint {:x}, plastic work {}",
        elapsed.as_secs_f64() * 1000.0 / 480.0,
        worst.as_secs_f64() * 1000.0,
        digest.finish(),
        world.stats().plastic_work
    );
}
