use super::*;

#[test]
fn authored_bounciness_survives_refinement_and_legacy_objects_keep_old_response() {
    let mut asset = crate::ObjectAsset::preset(crate::ObjectPreset::Rock);
    asset.refinement = 0;
    asset.cohesion = 1.0;
    let mut legacy = serde_json::to_value(&asset).unwrap();
    legacy.as_object_mut().unwrap().remove("restitution");
    assert_eq!(
        crate::ObjectAsset::from_json(&legacy.to_string()).unwrap(),
        asset
    );
    for invalid in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
        asset.restitution = invalid;
        assert!(asset.mesh().is_err());
    }
    asset.restitution = 0.7;
    for (restitution, speed) in [(1.1, 16.0), (-0.1, 16.0), (0.64, 0.0), (0.64, f32::NAN)] {
        asset.soft_bounce = Some(crate::SoftBounce { restitution, speed });
        assert!(asset.to_json().is_err());
    }
    asset.soft_bounce = Some(crate::SoftBounce::default());
    let mut world = World::from_object(&asset).unwrap();
    world.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..120 {
        world.step(&[]);
    }
    assert!(world.pieces.iter().any(|p| p.refinement > 0));
    assert!(world.stats().broken > 0);
    for p in world
        .pieces
        .iter()
        .filter(|p| p.active && p.object.is_some())
    {
        assert_eq!(p.restitution, 0.7);
        assert_eq!(world.physics.colliders[p.collider].restitution(), 0.7);
        assert_eq!(p.soft_bounce, asset.soft_bounce);
        assert!(
            world.physics.colliders[p.collider]
                .active_hooks()
                .contains(ActiveHooks::MODIFY_SOLVER_CONTACTS)
        );
    }
}

#[test]
fn authored_objects_preserve_mass_and_remain_stable_until_hit() {
    for preset in crate::ObjectPreset::ALL {
        let asset = crate::ObjectAsset::preset(preset);
        let mesh = asset.mesh().unwrap();
        let mut world = World::from_object(&asset).unwrap();
        assert_eq!(world.stats().cells, mesh.triangles.len());
        assert_eq!(world.stats().chunks, 1);
        assert!(world.physics.impulse_joints.is_empty());
        let mass: f32 = world
            .pieces
            .iter()
            .filter(|p| p.material == asset.surface.material())
            .map(|p| p.mass())
            .sum();
        assert!((mass - mesh.area * asset.density * asset.thickness).abs() < mass * 1e-5);
        let before: Vec<_> = world.bodies().map(|b| b.position).collect();
        for _ in 0..240 {
            world.step(&[]);
        }
        assert_eq!(
            world.stats().broken,
            0,
            "{} at rest: {:?}",
            asset.name,
            world.stats()
        );
        for (body, position) in world.bodies().zip(before) {
            assert!(body.position.distance(position) < 0.001);
        }
        world.step(&[Command::Fire(Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::new(3.0, 0.0),
            mass: 6.0,
            speed: 220.0,
        })]);
        for _ in 0..180 {
            world.step(&[]);
        }
        assert_eq!(world.stats().explosions, 1);
        assert_eq!(world.stats().shells, 0);
        assert!(
            world.stats().broken > 0,
            "{}: {:?}",
            asset.name,
            world.stats()
        );
        assert!(
            world
                .bodies()
                .all(|b| b.position.is_finite() && b.velocity.is_finite())
        );
    }
}

#[test]
fn authored_friction_and_strength_change_physical_response() {
    let mut low = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
    low.friction = 0.0;
    low.tension = 1000.0;
    low.compression = 1000.0;
    low.shear = 1000.0;
    let mut high = low.clone();
    high.friction = 1.0;
    high.tension = 1e9;
    high.compression = 1e9;
    high.shear = 1e9;
    let mut a = World::from_object(&low).unwrap();
    let mut b = World::from_object(&high).unwrap();
    for world in [&mut a, &mut b] {
        for piece in &world.pieces {
            if piece.material == Material::Panel {
                world.physics.bodies[piece.body].set_linvel(Vector::X, true);
            }
        }
        world.ground_resistance(0.05);
    }
    let speed = |w: &World| {
        w.pieces
            .iter()
            .filter(|p| p.material == Material::Panel)
            .map(|p| w.physics.bodies[p.body].linvel().x)
            .sum::<f32>()
            / 32.0
    };
    assert!((speed(&a) - 1.0).abs() < 1e-5);
    assert!((speed(&b) - 0.5095).abs() < 1e-4);
    for world in [&mut a, &mut b] {
        world.step(&[Command::Fire(Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::new(3.0, 0.0),
            mass: 6.0,
            speed: 220.0,
        })]);
        for _ in 0..240 {
            world.step(&[]);
        }
    }
    assert!(a.stats().broken > b.stats().broken);
    assert_eq!(b.stats().broken, 0);
}

#[test]
fn authored_supports_preserve_every_edge_orientation_at_rest() {
    for preset in crate::ObjectPreset::ALL {
        let mut asset = crate::ObjectAsset::preset(preset);
        for edge in 0..asset.outline.len() {
            asset.pinned_edge = Some(edge);
            let mut world = World::from_object(&asset).unwrap();
            for _ in 0..120 {
                world.step(&[]);
            }
            assert_eq!(world.stats().broken, 0, "{} edge {edge}", asset.name);
            assert!(
                world
                    .bodies()
                    .all(|b| b.position.distance(b.previous_position) < 0.001)
            );
        }
    }
}

fn run_shot(scene: Scene, mass: f32, speed: f32, y: f32) -> World {
    let mut world = World::new(scene);
    world.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, y),
        mass,
        speed,
    })]);
    for _ in 0..240 {
        world.step(&[]);
    }
    world
}

#[test]
fn structures_remain_intact_and_motionless_at_rest() {
    for scene in Scene::ALL {
        let mut world = World::new(scene);
        let positions: Vec<_> = world.bodies().map(|b| b.position).collect();
        for _ in 0..600 {
            world.step(&[]);
        }
        assert_eq!(world.stats().broken, 0, "{scene:?}");
        for (body, initial) in world.bodies().zip(&positions) {
            assert!(body.position.distance(*initial) < 0.005, "{scene:?}");
        }
        assert!(world.stats().kinetic_energy < 0.01);
    }
}

#[test]
fn impact_breaks_connections_before_small_debris_expires() {
    let initial = World::new(Scene::Masonry);
    let world = run_shot(Scene::Masonry, 6.0, 220.0, 0.0);
    assert!(world.stats().broken > 0, "{:?}", world.stats());
    assert_eq!(world.stats().cleared, 0);
    assert!(world.stats().chunks > initial.stats().chunks);
    assert!(
        world
            .bodies()
            .zip(initial.bodies())
            .any(|(body, original)| body.material == Material::Masonry
                && body.position.distance(original.position) > 0.05)
    );
    for body in world.bodies() {
        assert!(body.position.is_finite() && body.velocity.is_finite() && body.angle.is_finite());
    }
}

#[test]
fn gentle_mechanical_load_does_not_break_mortar() {
    let mut world = World::new(Scene::Masonry);
    let piece = world
        .pieces
        .iter()
        .find(|p| p.material == Material::Masonry)
        .unwrap()
        .body;
    world.physics.bodies[piece].apply_impulse(Vector::new(0.5, 0.0), true);
    for _ in 0..120 {
        world.step(&[]);
    }
    assert_eq!(world.stats().broken, 0);
}

#[test]
fn identical_command_logs_reproduce_identical_state() {
    let a = run_shot(Scene::Masonry, 6.0, 220.0, 1.0);
    let b = run_shot(Scene::Masonry, 6.0, 220.0, 1.0);
    assert_eq!(a.stats().broken, b.stats().broken);
    for (a, b) in a.bodies().zip(b.bodies()) {
        assert_eq!(a.position, b.position);
        assert_eq!(a.velocity, b.velocity);
        assert_eq!(a.angle, b.angle);
    }
}

#[test]
fn shot_and_recoil_conserve_linear_momentum() {
    let mut world = World::new(Scene::LooseBlocks);
    world.fire(
        Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::new(0.0, 2.0),
            mass: 6.0,
            speed: 220.0,
        },
        &mut Vec::new(),
    );
    let momentum = world
        .physics
        .bodies
        .iter()
        .filter(|(_, b)| b.is_dynamic())
        .fold(Vector::ZERO, |sum, (_, body)| {
            sum + body.linvel() * body.mass()
        });
    let momentum = momentum
        + world
            .shells
            .iter()
            .fold(Vector::ZERO, |sum, s| sum + s.velocity * s.mass);
    assert!(momentum.length() < 0.001, "{momentum:?}");
}

#[test]
fn structural_reaction_matches_known_static_load() {
    let mut world = World::new(Scene::LooseBlocks);
    let a = world.add_box(
        Vector::new(-2.0, -3.0),
        Vector::splat(0.2),
        Material::Anchor,
        true,
    );
    let b = world.add_box(
        Vector::new(-1.0, -3.0),
        Vector::splat(0.2),
        Material::Panel,
        false,
    );
    world.connect(
        a,
        b,
        Vector::new(-1.5, -3.0),
        Vector::X,
        0.4,
        Material::Masonry,
    );
    world.rebuild_chunks();
    let event = world.next_impact();
    world.loads.push(loads::Load {
        piece: b,
        point: Vector::new(-1.0, -3.0),
        impulse: Vector::new(2000.0 * TICK_DT, 0.0),
        energy: 10.0,
        apply: true,
        event,
    });
    world.process_loads(TICK_DT, &mut Vec::new());
    let ratio = world.bonds.last().unwrap().load;
    let expected = Material::Masonry
        .strength()
        .utilization(2000.0, 0.0, 0.0, 0.4, DEPTH);
    assert!(
        (ratio - expected).abs() < 0.001,
        "ratio {ratio}, expected {expected}"
    );
    assert!(world.bonds.last().unwrap().active);
}

#[test]
fn floor_resistance_never_adds_energy_or_reverses_motion() {
    for friction in [0.0, 0.09, 0.4] {
        for sign in [-1.0, 1.0] {
            let mut world = World::new(Scene::LooseBlocks);
            let velocity = Vector::new(sign * 0.002, 0.0);
            let id = world.add_ball(
                Vector::new(-3.0, -5.0),
                0.1,
                10.0,
                Material::Projectile,
                velocity,
            );
            world.pieces[id].floor_friction = friction;
            let body = world.pieces[id].body;
            world.physics.bodies[body].set_angvel(sign * 4.0, true);
            let before = world.physics.bodies[body].kinetic_energy();
            world.ground_resistance(0.1);
            let b = &world.physics.bodies[body];
            assert!(b.kinetic_energy() <= before);
            if friction == 0.0 {
                assert_eq!(b.linvel(), velocity);
                assert_eq!(b.angvel(), sign * 4.0);
            } else {
                assert_eq!(b.linvel(), Vector::ZERO);
                assert!(b.angvel() * sign > 0.0 && b.angvel().abs() < 4.0);
                // Even the low-friction barrel should settle within two seconds
                // from 4 rad/s, with no sign flip or residual spin at rest.
                for _ in 0..240 {
                    world.ground_resistance(TICK_DT);
                }
                assert_eq!(world.physics.bodies[body].angvel(), 0.0);
                assert_eq!(world.physics.bodies[body].kinetic_energy(), 0.0);
            }
        }
    }
}

#[test]
fn invalid_shots_cannot_poison_the_world() {
    let mut world = World::new(Scene::Masonry);
    for shot in [
        Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::splat(f32::NAN),
            mass: 1.0,
            speed: 1.0,
        },
        Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::ZERO,
            mass: f32::INFINITY,
            speed: 1.0,
        },
        Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: world.launcher_position(),
            mass: 1.0,
            speed: 1.0,
        },
    ] {
        world.step(&[Command::Fire(shot)]);
    }
    assert_eq!(world.stats().shots, 0);
}

#[test]
fn seam_capacity_scales_with_area_and_bending_section() {
    let strength = Material::Masonry.strength();
    let thin = strength.utilization(1_000.0, 0.0, 0.0, 0.5, DEPTH);
    let thick = strength.utilization(1_000.0, 0.0, 0.0, 1.0, DEPTH);
    assert!((thin / thick - 2.0).abs() < 0.0001);
    let narrow = strength.utilization(0.0, 0.0, 100.0, 0.5, DEPTH);
    let wide = strength.utilization(0.0, 0.0, 100.0, 1.0, DEPTH);
    assert!((narrow / wide - 4.0).abs() < 0.0001);
}

#[test]
fn cantilever_transmits_bending_load_to_its_neck() {
    let mut world = World::new(Scene::Cantilever);
    // Distribute a lateral load over the far panel, so this verifies load
    // transfer rather than demanding global collapse from a local explosion.
    let loaded: Vec<_> = world
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.material == Material::Panel && p.previous_position.y > 0.0)
        .map(|(i, _)| i)
        .collect();
    let event = world.next_impact();
    for &id in &loaded {
        world.loads.push(loads::Load {
            piece: id,
            point: world.piece_pose(id).translation,
            impulse: Vector::new(8000.0 * TICK_DT / loaded.len() as f32, 0.0),
            energy: 100.0,
            apply: true,
            event,
        });
    }
    let mut fractures = Vec::new();
    world.process_loads(TICK_DT, &mut fractures);
    assert!(
        fractures
            .iter()
            .any(|e| matches!(e, Event::Fractured { position, .. } if position.y < -2.4)),
        "neck must carry the bending load"
    );
}

#[test]
fn repeated_shells_open_a_localized_breach_in_anchored_masonry() {
    let initial = World::new(Scene::Masonry);
    let mut world = World::new(Scene::Masonry);
    for _ in 0..3 {
        world.step(&[Command::Fire(Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::new(3.0, 0.0),
            mass: 6.0,
            speed: 220.0,
        })]);
        for _ in 0..90 {
            world.step(&[]);
        }
    }
    let moved = world
        .bodies()
        .filter(|body| {
            body.material == Material::Masonry
                && body
                    .position
                    .distance(initial.pieces[body.origin].previous_position)
                    > 0.25
        })
        .count();
    assert!(
        moved >= 2,
        "expected displaced rubble, got {moved} pieces, {:?}",
        world.stats()
    );
    assert!(
        world.stats().bonds > initial.stats().bonds / 2,
        "a local breach should leave most connections intact"
    );
}

#[test]
fn ccd_stops_a_fast_ball_at_a_thin_barrier() {
    let mut world = World::new(Scene::LooseBlocks);
    world.add_box(
        Vector::new(-2.0, -5.0),
        Vector::new(0.025, 0.6),
        Material::Anchor,
        true,
    );
    let id = world.add_ball(
        Vector::new(-2.4, -5.0),
        0.06,
        10.0,
        Material::Projectile,
        Vector::new(100.0, 0.0),
    );
    let handle = world.pieces[id].body;
    for _ in 0..12 {
        world.step(&[]);
        assert!(
            world.physics.bodies[handle].translation().x < -2.0,
            "ball tunneled through the 5 cm barrier"
        );
    }
}

#[test]
fn shell_detonates_once_on_a_thin_barrier_and_is_consumed() {
    let mut world = World::new(Scene::LooseBlocks);
    world.add_box(
        Vector::new(-4.5, 0.0),
        Vector::new(0.025, 0.6),
        Material::Anchor,
        true,
    );
    let mut impacts = Vec::new();
    for tick in 0..120 {
        let commands = if tick == 0 {
            vec![Command::Fire(Shot {
                ammunition: crate::Ammunition::HighExplosive,
                target: Vector::new(3.0, 0.0),
                mass: 6.0,
                speed: 400.0,
            })]
        } else {
            vec![]
        };
        for event in world.step(&commands) {
            if let Event::Exploded { position, .. } = event {
                impacts.push(position);
            }
        }
    }
    assert_eq!(impacts.len(), 1);
    assert!((impacts[0].x + 4.525).abs() < 0.01, "{:?}", impacts);
    assert_eq!(world.stats().shells, 0);
    assert_eq!(world.stats().explosions, 1);
}

#[test]
fn barrel_cannot_fire_through_nearby_wall() {
    let mut world = World::new(Scene::Masonry);
    world.add_box(
        Vector::new(-5.6, 0.0),
        Vector::new(0.025, 0.6),
        Material::Anchor,
        true,
    );
    let events = world.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::ZERO,
        mass: 6.0,
        speed: 220.0,
    })]);
    assert_eq!(world.stats().shots, 0);
    assert!(events.iter().any(|e| matches!(e, Event::MuzzleBlocked)));
}

#[test]
fn tiny_disconnected_debris_clears_but_intact_structure_remains() {
    let initial = World::new(Scene::Masonry).stats().cells;
    let mut world = run_shot(Scene::Masonry, 6.0, 220.0, 0.0);
    for _ in 0..720 {
        world.step(&[]);
    }
    assert!(world.stats().cleared > 0, "{:?}", world.stats());
    assert_eq!(
        world.stats().cells + world.stats().cleared,
        initial + world.stats().refinements * 3
    );
    assert!(
        world.stats().cells > initial / 2,
        "large connected sections must survive"
    );
    assert!(
        world
            .bodies()
            .all(|b| b.position.is_finite() && b.velocity.is_finite())
    );
}

#[test]
fn track_controls_drive_turn_and_brake() {
    let mut world = World::new(Scene::Masonry);
    let initial = world.launcher_position();
    for _ in 0..90 {
        world.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
    }
    let driven = world.launcher_position();
    assert!(
        (0.3..0.65).contains(&(driven.x - initial.x)),
        "{initial:?} -> {driven:?}"
    );
    assert!((driven.y - initial.y).abs() < 0.05);
    for _ in 0..120 {
        world.step(&[]);
    }
    let stopped = world.launcher_position();
    assert!(
        stopped.distance(driven) < 0.25,
        "braking distance {}",
        stopped.distance(driven)
    );
    assert!(world.track_speeds().iter().all(|s| s.abs() < 0.02));
    let before = world.physics.bodies[world.pieces[world.launcher].body]
        .rotation()
        .angle();
    for _ in 0..120 {
        world.step(&[Command::Tracks {
            left: 1.0,
            right: -1.0,
        }]);
    }
    let body = &world.physics.bodies[world.pieces[world.launcher].body];
    assert!(
        (0.2..1.3).contains(&(body.rotation().angle() - before)),
        "differential steering must rotate clockwise"
    );
    assert!(
        body.translation().distance(stopped) < 0.3,
        "opposed tracks pivot in place"
    );
}

#[test]
fn recoil_is_arrested_by_track_brakes() {
    let mut world = World::new(Scene::Masonry);
    let initial = world.launcher_position();
    world.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..120 {
        world.step(&[]);
    }
    assert!(
        world.launcher_position().distance(initial) < 0.04,
        "recoil displacement {:?}",
        world.launcher_position() - initial
    );
    assert!(world.track_speeds().iter().all(|s| s.abs() < 0.01));
}

#[test]
fn repeated_shots_respect_reload_and_never_accumulate_projectile_bodies() {
    let mut world = World::new(Scene::Masonry);
    let command = Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    });
    for _ in 0..120 {
        world.step(&[command]);
    }
    assert_eq!(world.stats().shots, 2);
    for _ in 0..480 {
        world.step(&[]);
    }
    assert_eq!(world.stats().explosions, 2);
    assert_eq!(world.stats().shells, 0);
    assert!(!world.bodies().any(|b| b.material == Material::Projectile));
}

#[test]
fn tank_turn_rate_and_release_overshoot_are_limited() {
    let mut world = World::new(Scene::Masonry);
    let handle = world.pieces[world.launcher].body;
    for _ in 0..240 {
        world.step(&[Command::Tracks {
            left: 1.0,
            right: -1.0,
        }]);
        assert!(world.physics.bodies[handle].angvel().abs() < 1.3);
    }
    let before = world.physics.bodies[handle].rotation().angle();
    for _ in 0..30 {
        world.step(&[]);
    }
    let tank = &world.physics.bodies[handle];
    let extra = (tank.rotation().angle() - before).abs();
    assert!(
        extra < 0.08,
        "turning coast: {} degrees",
        extra.to_degrees()
    );
    assert!(
        tank.angvel().abs() < 0.01,
        "remaining rotation {}",
        tank.angvel()
    );
}

fn rock_components(world: &World, rock: usize) -> Vec<Vec<usize>> {
    let start = 5 + rock * 7;
    let mut remaining: Vec<_> = world
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            p.active && p.material == Material::Rock && (start..start + 7).contains(&p.origin)
        })
        .map(|(id, _)| id)
        .collect();
    let mut result = Vec::new();
    while let Some(seed) = remaining.pop() {
        let mut component = vec![seed];
        let mut i = 0;
        while i < component.len() {
            let current = component[i];
            for bond in world.bonds.iter().filter(|b| b.active) {
                let next = if bond.a == current {
                    bond.b
                } else if bond.b == current {
                    bond.a
                } else {
                    continue;
                };
                if let Some(index) = remaining.iter().position(|&id| id == next) {
                    remaining.swap_remove(index);
                    component.push(next);
                }
            }
            i += 1;
        }
        result.push(component);
    }
    result
}

#[test]
fn rock_hit_leaves_substantial_chunks_and_does_not_cascade_across_field() {
    let mut world = World::new(Scene::LooseBlocks);
    world.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..960 {
        world.step(&[]);
    }
    let components: Vec<_> = (0..15).map(|i| rock_components(&world, i)).collect();
    let intact = components.iter().filter(|c| c.len() == 1).count();
    let target_sizes: Vec<f32> = components[6]
        .iter()
        .map(|c| c.iter().map(|&id| world.pieces[id].shape.area()).sum())
        .collect();
    println!(
        "rock test: {} intact, target {:?}, {:?}",
        intact,
        target_sizes,
        world.stats()
    );
    assert!(components[6].len() >= 2, "the struck rock should split");
    assert!(
        target_sizes
            .iter()
            .filter(|&&area| area >= DEBRIS_AREA)
            .count()
            >= 2,
        "expected substantial chunks: {target_sizes:?}"
    );
    assert!(
        intact >= 11,
        "only nearby rocks should fracture; {intact} intact"
    );
    assert!(
        world
            .pieces
            .iter()
            .filter(|p| !p.active && p.debris_remaining.is_some() && p.material == Material::Rock)
            .map(|p| p.shape.area())
            .sum::<f32>()
            <= 0.225,
        "a rock field should not turn to disappearing dust"
    );
    assert_eq!(
        world.stats().explosions,
        1,
        "rocks never produce secondary explosions"
    );
}

#[test]
fn oblique_rock_hits_preserve_the_rest_of_the_field() {
    for y in [-3.6, -1.8, 1.8, 3.6] {
        let mut world = World::new(Scene::LooseBlocks);
        world.step(&[Command::Fire(Shot {
            ammunition: crate::Ammunition::HighExplosive,
            target: Vector::new(0.8, y),
            mass: 6.0,
            speed: 220.0,
        })]);
        for _ in 0..960 {
            world.step(&[]);
        }
        let intact = (0..15)
            .filter(|&i| {
                let c = rock_components(&world, i);
                c.len() == 1
            })
            .count();
        assert!(intact >= 10, "at y={y}: only {intact} intact rocks");
        assert!(
            world
                .pieces
                .iter()
                .filter(|p| !p.active
                    && p.debris_remaining.is_some()
                    && p.material == Material::Rock)
                .map(|p| p.shape.area())
                .sum::<f32>()
                <= 0.225,
            "at y={y}: {:?}",
            world.stats()
        );
        assert_eq!(world.stats().explosions, 1);
    }
}

#[test]
fn one_track_input_keeps_the_other_track_braked() {
    let mut world = World::new(Scene::Masonry);
    for _ in 0..120 {
        world.step(&[Command::Tracks {
            left: 1.0,
            right: 0.0,
        }]);
    }
    let travel = world.track_travel();
    assert!(
        travel[0] > 0.4,
        "left track should propel the tank: {travel:?}"
    );
    assert!(
        travel[1].abs() < 0.15,
        "right track should remain braked: {travel:?}"
    );
}
