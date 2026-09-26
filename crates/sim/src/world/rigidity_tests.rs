use super::*;

#[test]
fn central_attachment_survives_refinement_without_anchoring_detached_chips() {
    let mut asset = crate::LibraryPreset::Rock.asset();
    asset.outline = vec![[-0.45, -0.3], [0.45, -0.3], [0.0, 0.3]];
    asset.layout = None;
    let mut world = World::from_object(&asset).unwrap();
    let anchor = |w: &World| {
        w.pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.active && p.ground_anchor.is_some())
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    };
    let original = anchor(&world);
    assert_eq!(original.len(), 1);
    let id = original[0];
    let point = world
        .piece_pose(id)
        .transform_point(world.pieces[id].ground_anchor.unwrap());
    let mass = |w: &World| {
        w.pieces
            .iter()
            .filter(|p| p.active && p.object == Some(0))
            .map(Piece::mass)
            .sum::<f32>()
    };
    let before = mass(&world);
    let event = world.next_impact();
    world.loads.push(loads::Load {
        piece: id,
        point,
        impulse: Vector::X * 1e6,
        energy: 1e9,
        apply: true,
        event,
    });
    world.refine_loaded_cells();
    world.loads.clear();
    assert_eq!(world.refinements, 1);
    let after = anchor(&world);
    assert_eq!(after.len(), 1);
    assert_ne!(after[0], id);
    let child = after[0];
    let attached = world
        .piece_pose(child)
        .transform_point(world.pieces[child].ground_anchor.unwrap());
    assert!(point.distance(attached) < 1e-5);
    assert!((mass(&world) - before).abs() < before * 1e-5);
    assert!(world.physics.bodies[world.pieces[child].body].is_fixed());
    // Sever the actual material interfaces surrounding the attached child.
    for bond in &mut world.bonds {
        if bond.a == child || bond.b == child {
            bond.active = false;
        }
    }
    world.rebuild_chunks();
    assert!(world.physics.bodies[world.pieces[child].body].is_fixed());
    assert!(
        world
            .pieces
            .iter()
            .filter(|p| p.active && p.object == Some(0) && p.ground_anchor.is_none())
            .all(|p| world.physics.bodies[p.body].is_dynamic())
    );
    assert!(world.pieces[child].shape.area() < DEBRIS_AREA);
    world.topology_changed = true;
    world.cleanup_debris();
    assert!(
        world.pieces[child].debris_remaining.is_some(),
        "a tiny anchored stump must remain clearable"
    );
    for _ in 0..500 {
        world.cleanup_debris();
    }
    assert!(!world.pieces[child].active);
}

fn totals(world: &World) -> [f64; 5] {
    let mut total = [0.0; 5];
    for (_, b) in world.physics.bodies.iter().filter(|(_, b)| b.is_dynamic()) {
        let m = b.mass() as f64;
        let v = b.linvel();
        let p = b.center_of_mass();
        let inertia = 1.0 / b.mass_properties().effective_world_inv_inertia as f64;
        total[0] += m;
        total[1] += m * v.x as f64;
        total[2] += m * v.y as f64;
        total[3] += m * p.perp_dot(v) as f64 + inertia * b.angvel() as f64;
        total[4] += b.kinetic_energy() as f64;
    }
    total
}

fn close(a: [f64; 5], b: [f64; 5]) {
    for i in 0..5 {
        assert!(
            (a[i] - b[i]).abs() < a[i].abs().max(1.0) * 3e-5,
            "component {i}: {a:?} -> {b:?}"
        );
    }
}

#[test]
fn refining_a_moving_rotating_cell_preserves_mass_momentum_and_energy() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
    asset.refinement = 0;
    let mut world = World::from_object(&asset).unwrap();
    let id = world.pieces.iter().position(|p| p.destructible()).unwrap();
    let h = world.pieces[id].body;
    world.physics.bodies[h].set_linvel(Vector::new(2.0, -1.0), true);
    world.physics.bodies[h].set_angvel(1.3, true);
    let before = totals(&world);
    let event = world.next_impact();
    world.loads.push(loads::Load {
        piece: id,
        point: world.piece_pose(id).translation,
        impulse: Vector::new(2000.0, 0.0),
        energy: 100_000.0,
        apply: true,
        event,
    });
    world.refine_loaded_cells();
    assert_eq!(world.stats().refinements, 1);
    assert_eq!(world.stats().chunks, 1);
    assert_eq!(world.stats().cells, 5);
    close(before, totals(&world));
    // All original interfaces remain, with exactly the same total face width.
    let before_split = totals(&world);
    for b in &mut world.bonds {
        b.active = false;
    }
    world.rebuild_chunks();
    assert_eq!(world.stats().chunks, 5);
    close(before_split, totals(&world));
    let live: std::collections::BTreeSet<_> = world
        .pieces
        .iter()
        .filter(|p| p.active)
        .map(|p| p.body.into_raw_parts())
        .collect();
    assert_eq!(
        live.len(),
        world.physics.bodies.len(),
        "no parent bodies or colliders survive refinement"
    );
}

#[test]
fn material_remains_rigid_and_mass_is_independent_of_editor_detail() {
    let mut reference = None;
    for detail in 0..=4 {
        let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Rock);
        asset.refinement = detail;
        asset.tension = 1e9;
        asset.compression = 1e9;
        asset.shear = 1e9;
        let mut world = World::from_object(&asset).unwrap();
        let id = world.pieces.iter().position(|p| p.destructible()).unwrap();
        let positions: Vec<_> = world
            .bodies()
            .filter(|b| b.material == Material::Rock)
            .map(|b| (b.id, b.position))
            .collect();
        let event = world.next_impact();
        world.loads.push(loads::Load {
            piece: id,
            point: Vector::new(1.6, -0.5),
            impulse: Vector::new(800.0, 100.0),
            energy: 10_000.0,
            apply: true,
            event,
        });
        world.process_loads(TICK_DT, &mut Vec::new());
        assert_eq!(world.stats().broken, 0);
        assert_eq!(
            world.stats().unconverged,
            0,
            "detail {detail}: {:?}",
            world.stats()
        );
        assert_eq!(world.stats().chunks, 1);
        assert_eq!(world.stats().bodies, 2);
        if let Some(r) = reference {
            close(r, totals(&world));
        } else {
            reference = Some(totals(&world));
        }
        for _ in 0..60 {
            world.step(&[]);
        }
        let first = positions[0];
        let origin = world.piece_pose(first.0).translation;
        for &(id, initial) in &positions {
            let distance = world.piece_pose(id).translation.distance(origin);
            assert!(
                (distance - initial.distance(first.1)).abs() < 2e-5,
                "detail {detail}: cell {id} flexed"
            );
        }
        assert!(world.physics.impulse_joints.is_empty());
    }
}

#[test]
fn an_isolated_large_triangle_can_be_broken_into_clearable_debris() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Rock);
    asset.outline = vec![[-0.8, -0.6], [0.8, -0.6], [0.0, 0.6]];
    asset.refinement = 0;
    asset.cohesion = 1.0;
    let mut world = World::from_object(&asset).unwrap();
    for _ in 0..8 {
        let Some(id) = world
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.active && p.destructible() && p.shape.area() >= DEBRIS_AREA)
            .max_by(|(_, a), (_, b)| a.shape.area().total_cmp(&b.shape.area()))
            .map(|(i, _)| i)
        else {
            break;
        };
        let event = world.next_impact();
        let point = world.piece_pose(id).translation + Vector::new(-0.02, 0.01);
        world.loads.push(loads::Load {
            piece: id,
            point,
            impulse: Vector::new(1500.0, 0.0),
            energy: 300_000.0,
            apply: true,
            event,
        });
        let before = totals(&world)[4] + world.fracture_work;
        world.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
        assert!(totals(&world)[4] + world.fracture_work <= before + 300_000.0 + 1.0);
        world.cleanup_debris();
    }
    assert!(world.stats().refinements > 1);
    assert!(world.stats().broken > 0);
    for _ in 0..600 {
        world.step(&[]);
    }
    assert!(world.stats().cleared > 0, "{:?}", world.stats());
    assert!(
        world
            .pieces
            .iter()
            .filter(|p| p.active && p.destructible())
            .all(|p| p.shape.area() < DEBRIS_AREA),
        "large unbreakable triangle remains: {:?}",
        world.stats()
    );
}

#[test]
fn refinement_alone_does_not_break_bonds_or_spend_fracture_energy() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
    asset.refinement = 0;
    let mut w = World::from_object(&asset).unwrap();
    let id = w.pieces.iter().position(|p| p.destructible()).unwrap();
    let event = w.next_impact();
    w.loads.push(loads::Load {
        piece: id,
        point: w.piece_pose(id).translation,
        impulse: Vector::new(5000.0, 0.0),
        energy: 1000.0,
        apply: true,
        event,
    });
    w.refine_loaded_cells();
    assert_eq!(w.broken, 0);
    assert_eq!(w.fracture_work, 0.0);
    assert_eq!(w.loads[0].energy, 1000.0);
    let count = w.refinements;
    w.refine_loaded_cells();
    assert_eq!(
        w.refinements, count,
        "same impact must not recursively refine its descendants"
    );
    // A contact may intervene while the original blast pulse is still active.
    // Descendants must remember both sources, not only the most recent one.
    w.loads[0].event = w.next_impact();
    w.refine_loaded_cells();
    assert_eq!(w.refinements, count + 1);
    w.loads[0].event = event;
    w.refine_loaded_cells();
    assert_eq!(
        w.refinements,
        count + 1,
        "an intervening impact must not reset the original source guard"
    );
}

#[test]
fn refinement_preserves_support_faces_and_does_not_heal_existing_cracks() {
    use std::collections::BTreeMap;
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
    asset.refinement = 1;
    asset.pinned_edge = Some(0);
    for p in &mut asset.outline {
        *p = (Rotation::new(0.61) * Vector::from(*p)).into();
    }
    let mut world = World::from_object(&asset).unwrap();
    let parent = world
        .bonds
        .iter()
        .find(|b| !world.pieces[b.a].destructible())
        .unwrap()
        .b;
    let cracked = world
        .bonds
        .iter()
        .position(|b| {
            b.a == parent && world.pieces[b.b].destructible()
                || b.b == parent && world.pieces[b.a].destructible()
        })
        .unwrap();
    world.bonds[cracked].active = false;
    world.rebuild_chunks();
    let faces = |w: &World| {
        let mut result = BTreeMap::new();
        for b in w.bonds.iter().filter(|b| b.active) {
            let (a, z) = (w.pieces[b.a].origin, w.pieces[b.b].origin);
            if a != z {
                *result.entry((a.min(z), a.max(z))).or_insert(0.0_f32) += b.width;
            }
        }
        result
    };
    let before = faces(&world);
    let mass: f32 = world
        .pieces
        .iter()
        .filter(|p| p.active && p.destructible())
        .map(Piece::mass)
        .sum();
    let chunks = world.stats().chunks;
    let event = world.next_impact();
    world.loads.push(loads::Load {
        piece: parent,
        point: world.piece_pose(parent).translation,
        impulse: Vector::new(100_000.0, 0.0),
        energy: 100_000.0,
        apply: true,
        event,
    });
    world.refine_loaded_cells();
    assert_eq!(world.refinements, 1);
    let after = faces(&world);
    assert_eq!(before.len(), after.len(), "no faces lost or cracks healed");
    for (key, width) in before {
        assert!((after[&key] - width).abs() < 1e-5);
    }
    let new_mass: f32 = world
        .pieces
        .iter()
        .filter(|p| p.active && p.destructible())
        .map(Piece::mass)
        .sum();
    assert!((mass - new_mass).abs() < mass * 1e-5);
    assert_eq!(world.stats().chunks, chunks);
}

#[test]
fn high_stress_without_available_work_cannot_create_cracks_or_motion() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Rock);
    asset.refinement = 0;
    let mut world = World::from_object(&asset).unwrap();
    let id = world.pieces.iter().position(|p| p.destructible()).unwrap();
    let event = world.next_impact();
    world.loads.push(loads::Load {
        piece: id,
        point: world.piece_pose(id).translation,
        impulse: Vector::new(100_000.0, 0.0),
        energy: 0.0,
        apply: true,
        event,
    });
    world.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
    assert!(world.stats().peak_load > 1.0);
    assert_eq!(world.stats().unconverged, 0);
    assert_eq!(world.stats().broken, 0);
    assert_eq!(world.stats().refinements, 0);
    assert_eq!(world.stats().kinetic_energy, 0.0);
    assert_eq!(world.fracture_work, 0.0);
}

#[test]
fn cached_stresses_rotate_with_the_chunk() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Rock);
    asset.refinement = 2;
    let mut world = World::from_object(&asset).unwrap();
    let id = world.pieces.iter().position(|p| p.destructible()).unwrap();
    let h = world.pieces[id].body;
    let original = *world.physics.bodies[h].position();
    let point = original.inverse_transform_point(world.piece_pose(id).translation);
    let mut reference = Vec::new();
    for angle in [0.0, 1.2, -2.4] {
        world.physics.bodies[h].set_rotation(Rotation::new(angle), true);
        let frame = *world.physics.bodies[h].position();
        for b in &mut world.bonds {
            b.load = 0.0;
        }
        let event = world.next_impact();
        world.loads.push(loads::Load {
            piece: id,
            point: frame.transform_point(point),
            impulse: frame.rotation * Vector::new(10.0, 3.0),
            energy: 0.0,
            apply: false,
            event,
        });
        world.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
        let reactions: Vec<_> = world.bonds.iter().map(|b| b.load).collect();
        if reference.is_empty() {
            reference = reactions;
        } else {
            for (actual, expected) in reactions.iter().zip(&reference) {
                assert!((actual - expected).abs() < expected.max(0.01) * 1e-3);
            }
        }
        assert_eq!(world.stats().unconverged, 0);
        assert_eq!(world.structural_cache.len(), 1);
    }
}

#[test]
fn contact_crack_budgets_fit_dissipation_and_impulses_are_not_applied_twice() {
    let mut world = World::new(Scene::LooseBlocks);
    world.add_box(
        Vector::new(-2.85, -5.0),
        Vector::new(0.1, 0.5),
        Material::Anchor,
        true,
    );
    let id = world.add_box(
        Vector::new(-3.2, -5.0),
        Vector::new(0.15, 0.4),
        Material::Rock,
        false,
    );
    world.physics.bodies[world.pieces[id].body].set_linvel(Vector::new(20.0, 0.0), true);
    let mut work = 0.0;
    for _ in 0..40 {
        let before = world.stats().kinetic_energy;
        let velocities = world.contact_velocities();
        world.physics.step();
        let after = world.stats().kinetic_energy;
        world.collect_contact_loads(&velocities, &mut Vec::new());
        let budget: f32 = world.loads.iter().map(|l| l.energy).sum();
        assert!(budget <= (before - after).max(0.0) + 1e-3);
        assert!(world.loads.iter().all(|l| !l.apply));
        work += budget;
        world.process_loads(TICK_DT / SUBSTEPS as f32, &mut Vec::new());
        assert_eq!(
            world.stats().kinetic_energy,
            after,
            "contact impulse was applied twice"
        );
    }
    assert!(work > 100.0, "test must include an energetic contact");
}

#[test]
fn hull_contact_reports_audio_events_without_stationary_contact_chatter() {
    let mut level = crate::level::Level::default();
    level.items.retain(|item| {
        !matches!(
            item,
            crate::level::LevelItem::Spawn {
                team: crate::level::Team::Blue,
                ..
            }
        )
    });
    level.items.push(crate::level::LevelItem::Spawn {
        team: crate::level::Team::Blue,
        position: [-5.0, 0.0],
        angle: 0.0,
    });
    level.items.push(crate::level::LevelItem::Solid {
        outline: vec![[-2.0, -4.0], [-1.0, -4.0], [-1.0, 4.0], [-2.0, 4.0]],
        position: [0.0, 0.0],
        angle: 0.0,
        cutouts: vec![],
    });
    let mut world = World::from_level(&level, crate::level::Team::Blue).unwrap();
    for _ in 0..5 {
        assert!(
            !world
                .step(&[])
                .iter()
                .any(|event| matches!(event, Event::Collided { .. }))
        );
    }
    let mut collided = false;
    for _ in 0..240 {
        collided |= world
            .step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }])
            .iter()
            .any(|event| matches!(event,Event::Collided {energy,..} if *energy > 80.0));
    }
    assert!(
        collided,
        "driving into fixed scenery produces a hull impact cue"
    );
}
