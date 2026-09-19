use super::*;
use crate::LibraryPreset as P;
#[test]
fn library_objects_roundtrip_and_remain_intact_at_rest() {
    for p in P::ALL {
        let a = p.asset();
        assert_eq!(
            a,
            crate::ObjectAsset::from_json(&a.to_json().unwrap()).unwrap()
        );
        let mut w = World::from_object(&a).unwrap();
        for _ in 0..120 {
            w.step(&[]);
        }
        assert_eq!(w.stats().broken, 0, "{}", p.label());
        assert_eq!(w.stats().explosions, 0);
        assert!(
            w.bodies()
                .filter(|b| b.material != Material::Launcher
                    && b.material != Material::Boundary
                    && b.material != Material::Anchor)
                .all(|b| b.color == a.color)
        );
    }
}
#[test]
fn explosive_damage_fires_once_and_reaches_neighbors() {
    let a = P::ExplosiveBarrel.asset();
    let mut neighbor = crate::ObjectAsset::workshop_preset(crate::ObjectPreset::Plate);
    neighbor.outline = vec![[-0.2, -0.8], [0.2, -0.8], [0.2, 0.8], [-0.2, 0.8]];
    neighbor.regenerate_grains(crate::GrainMode::Bricks, [0.4, 0.4], 7);
    neighbor.tension = 5_000.0;
    neighbor.compression = 10_000.0;
    neighbor.shear = 5_000.0;
    neighbor.toughness = Some(2.0);
    neighbor.cohesion = 1.0;
    let mut w = World::from_objects(&[
        (a, Vector::new(2.0, 0.0)),
        (neighbor, Vector::new(3.5, 0.0)),
    ])
    .unwrap();
    w.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(2.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..180 {
        w.step(&[]);
    }
    assert!(w.objects[0].spent, "barrel must deform and ignite");
    assert!(w.stats().plastic_work > 0.0);
    assert_eq!(w.stats().explosions, 2, "one shell plus one barrel");
    assert!(
        w.bonds.iter().any(|b| !b.active
            && w.pieces[b.a].object == Some(1)
            && w.pieces[b.b].object == Some(1)),
        "neighbor must fracture"
    );
    assert!(!w.pieces.iter().any(|p| p.active && p.object == Some(0)));
    for _ in 0..120 {
        w.step(&[]);
    }
    assert_eq!(w.stats().explosions, 2);
}
#[test]
fn barrel_moves_under_a_shove_and_buckles_when_pinned() {
    fn trial(backstop: bool) -> World {
        let a = P::Barrel.asset();
        let mut w = World::from_objects(&[(a, Vector::new(1.0, 0.0))]).unwrap();
        if backstop {
            w.add_box(
                Vector::new(1.9, 0.0),
                Vector::new(0.2, 1.4),
                Material::Anchor,
                true,
            );
        }
        w.configure_test_tank(4500.0, Vector::new(-0.8, 0.0), 0.8)
            .unwrap();
        for _ in 0..360 {
            w.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
        }
        w
    }
    let free = trial(false);
    assert_eq!(free.stats().broken, 0, "free barrel should be pushed away");
    assert!(
        free.bodies()
            .any(|b| b.color == P::Barrel.asset().color && b.position.x > 1.5)
    );
    let pinned = trial(true);
    assert!(
        pinned.stats().plastic_strain > 0.03,
        "pinning must allow buckling"
    );
    assert_eq!(pinned.stats().broken, 0, "deformable frames stay connected");
    assert_eq!(
        free.stats().plastic_work,
        0.0,
        "a free shove does not squeeze the barrel"
    );
    assert_eq!(pinned.stats().unconverged, 0);
}
#[test]
fn plain_barrel_resists_a_standard_shell_without_igniting() {
    let mut w = World::from_object(&P::Barrel.asset()).unwrap();
    w.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..120 {
        w.step(&[]);
    }
    assert_eq!(w.stats().explosions, 1);
    assert_eq!(w.stats().broken, 0);
}
#[test]
fn rock_chips_under_a_shell_but_retains_a_core_and_resists_ramming() {
    let a = P::Rock.asset();
    let area = a.mesh().unwrap().area;
    let mut shell = World::from_object(&a).unwrap();
    shell.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..360 {
        shell.step(&[]);
    }
    assert!(shell.stats().broken > 0);
    let largest = shell
        .chunk_members()
        .values()
        .map(|members| {
            members
                .iter()
                .filter(|&&i| shell.pieces[i].object == Some(0))
                .map(|&i| shell.pieces[i].shape.area())
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    assert!(
        largest > area * 0.6,
        "rock should keep a substantial core: {largest}/{area}"
    );
    let held: f32 = shell
        .pieces
        .iter()
        .filter(|p| p.active && p.object == Some(0) && shell.physics.bodies[p.body].is_fixed())
        .map(|p| p.shape.area())
        .sum();
    assert!(
        held > area * 0.6,
        "a shot should chip the perimeter, not sever the buried core: {held}/{area}"
    );
    let mut tank = World::from_object(&a).unwrap();
    tank.configure_test_tank(4500.0, Vector::new(-1.0, 0.0), 4.6)
        .unwrap();
    for _ in 0..360 {
        tank.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
    }
    assert_eq!(tank.stats().broken, 0);
    assert_eq!(shell.stats().unconverged + tank.stats().unconverged, 0);
}
#[test]
fn wall_breaks_into_coarser_chunks_under_tank_contact() {
    let a = P::Wall.asset();
    let mut counts = Vec::new();
    for tank in [false, true] {
        let mut w = World::from_object(&a).unwrap();
        if tank {
            w.configure_test_tank(4500.0, Vector::new(-1.0, 0.0), 4.6)
                .unwrap();
        } else {
            w.step(&[Command::Fire(Shot {
                ammunition: crate::Ammunition::HighExplosive,
                target: Vector::new(3.0, 0.0),
                mass: 6.0,
                speed: 220.0,
            })]);
        }
        for _ in 0..120 {
            w.step(if tank {
                &[Command::Tracks {
                    left: 1.0,
                    right: 1.0,
                }]
            } else {
                &[]
            });
        }
        assert!(w.stats().broken > 0);
        assert_eq!(w.stats().unconverged, 0);
        if tank {
            assert_eq!(w.stats().refinements, 0);
        }
        counts.push(w.stats().chunks);
    }
    assert!(
        counts[0] > counts[1] && counts[1] > 1,
        "shell/tank chunks: {counts:?}"
    );
}

#[test]
fn wall_splits_on_the_first_standard_shell_hit() {
    let asset = P::Wall.asset();
    let initial = asset.mesh().unwrap().area;
    let mut w = World::from_object(&asset).unwrap();
    w.step(&[Command::Fire(Shot {
        ammunition: crate::Ammunition::HighExplosive,
        target: Vector::new(3.0, 0.0),
        mass: 6.0,
        speed: 220.0,
    })]);
    for _ in 0..120 {
        w.step(&[]);
    }
    let largest = w
        .chunk_members()
        .values()
        .map(|members| {
            members
                .iter()
                .filter(|&&i| w.pieces[i].object == Some(0))
                .map(|&i| w.pieces[i].shape.area())
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    assert!(
        largest < initial * 0.6,
        "first shot must split the wall, not just crack its supports: {largest}/{initial}"
    );
    assert_eq!(w.stats().unconverged, 0);
}

#[test]
fn grounded_rock_cannot_be_pushed_or_turned_by_oblique_tank_impacts() {
    let asset = P::Rock.asset();
    for angle in [-0.6_f32, 0.0, 0.6] {
        let mut w = World::from_object(&asset).unwrap();
        let initial: Vec<_> = w
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.object == Some(0))
            .map(|(id, _)| (id, w.piece_pose(id)))
            .collect();
        let dir = Vector::new(angle.cos(), angle.sin());
        w.configure_test_tank_angle(4500.0, Vector::new(3.0, 0.0) - dir * 4.0, 4.6, angle)
            .unwrap();
        for _ in 0..600 {
            w.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
        }
        for (id, pose) in initial {
            assert!(w.pieces[id].active);
            assert!(
                w.physics.bodies[w.pieces[id].body].is_fixed(),
                "ram detached a stone at angle {angle}"
            );
            assert!(w.piece_pose(id).translation.distance(pose.translation) < 1e-5);
            assert!((w.piece_pose(id).rotation.angle() - pose.rotation.angle()).abs() < 1e-5);
        }
        assert_eq!(w.stats().unconverged, 0);
    }
}
#[test]
fn component_validation_and_library_preserve_authored_values() {
    let mut a = P::Barrel.asset();
    a.components.projectile_resistance = f32::NAN;
    assert!(a.mesh().is_err());
    a = P::ExplosiveBarrel.asset();
    a.components.explosive.as_mut().unwrap().radius = 100.0;
    assert!(a.mesh().is_err());
    for p in [
        crate::Plasticity {
            yield_stress: 0.0,
            ..Default::default()
        },
        crate::Plasticity {
            flow_rate: f32::INFINITY,
            ..Default::default()
        },
        crate::Plasticity {
            hardening: -1.0,
            ..Default::default()
        },
        crate::Plasticity {
            max_strain: 4.0,
            ..Default::default()
        },
    ] {
        let mut a = P::Barrel.asset();
        a.components.plasticity = Some(p);
        assert!(a.mesh().is_err());
    }
    let mut legacy = serde_json::to_value(P::Barrel.asset()).unwrap();
    legacy["components"]
        .as_object_mut()
        .unwrap()
        .remove("plasticity");
    assert!(
        crate::ObjectAsset::from_json(&legacy.to_string())
            .unwrap()
            .components
            .plasticity
            .is_none()
    );
    let mut library = crate::ObjectLibrary::default();
    let mut a = P::Rock.asset();
    library.save(a.clone());
    a.friction = 1.4;
    library.save(a.clone());
    let loaded = crate::ObjectLibrary::from_json(&library.to_json().unwrap()).unwrap();
    assert_eq!(loaded.objects, vec![a]);
}
