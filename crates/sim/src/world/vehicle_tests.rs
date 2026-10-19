use super::*;
use crate::level::{Level, Team};
fn world(kind: VehicleKind) -> World {
    let mut w = World::from_level(&Level::default(), Team::Blue).unwrap();
    w.enemy = None;
    w.apply_vehicle(w.launcher, kind);
    w
}
#[test]
fn deployment_is_required_after_each_death_and_cannot_change_a_living_vehicle() {
    let mut w = world(VehicleKind::Tank);
    w.require_vehicle_choice(w.launcher);
    for _ in 0..1200 {
        w.step(&[]);
    }
    assert!(!w.tank_alive());
    w.step(&[Command::Deploy(VehicleKind::Scout)]);
    w.step(&[]);
    assert!(w.tank_alive());
    assert_eq!(w.player_vehicle(), VehicleKind::Scout);
    w.step(&[Command::Deploy(VehicleKind::Mortar)]);
    assert_eq!(w.player_vehicle(), VehicleKind::Scout);
    w.tanks
        .iter_mut()
        .find(|t| t.id == w.launcher)
        .unwrap()
        .protection = 0.0;
    w.damage_tank(w.launcher, 1000.0, &mut vec![]);
    for _ in 0..1200 {
        w.step(&[]);
    }
    assert!(!w.tank_alive());
    w.step(&[Command::Deploy(VehicleKind::Mortar)]);
    w.step(&[]);
    assert!(w.tank_alive());
    assert_eq!(w.player_vehicle(), VehicleKind::Mortar);
    let body = &w.physics.bodies[w.pieces[w.launcher].body];
    assert!((body.mass() - VehicleKind::Mortar.mass()).abs() < 1.0);
}
#[test]
fn mortar_has_fixed_gun_and_long_narrow_sight_with_double_near_radius() {
    let mut w = world(VehicleKind::Mortar);
    let origin = w.launcher_position();
    for _ in 0..240 {
        w.step(&[Command::Aim(origin + Vector::Y * 20.0)]);
    }
    assert!(w.player_aim().angle.abs() < 0.001);
    let vision = w.player_vision(0.0);
    assert!(vision.contains(origin + Vector::X * 30.0));
    assert!(!vision.contains(origin + Vector::from_angle(0.65) * 15.0));
    assert!(vision.contains(origin - Vector::X * 5.5));
    assert!(!vision.contains(origin - Vector::X * 6.5));
    let radius = w.player_aim().mortar_impact_radius(20.0);
    w.step(&[Command::Fire(Shot::new(
        Ammunition::HighExplosive,
        origin + Vector::Y * 20.0,
    ))]);
    let shell = w.shells[0];
    assert!(shell.flight > 0.0);
    let landing = shell.position + shell.velocity * (shell.flight - shell.age);
    assert!(landing.distance(origin + Vector::X * 20.0) <= radius);
    assert!(
        landing.distance(origin + Vector::Y * 20.0) > 20.0,
        "gun stays fixed to the hull"
    );
}
#[test]
fn scout_rounds_damage_tanks_but_never_fracture_walls_or_take_specials() {
    let mut w = world(VehicleKind::Scout);
    let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
    let target = w.piece_pose(enemy).translation;
    for tick in 0..250 {
        w.step(&if tick % 24 == 0 {
            vec![Command::Fire(Shot::new(Ammunition::HighExplosive, target))]
        } else {
            vec![]
        });
    }
    assert!(w.tanks.iter().find(|t| t.id == enemy).unwrap().health < 100.0);
    let mut w = World::new(Scene::Masonry);
    w.register_tank(w.launcher, Team::Blue, w.launcher_position(), 0.0);
    w.apply_vehicle(w.launcher, VehicleKind::Scout);
    let active = w.pieces.iter().filter(|p| p.active).count();
    for tick in 0..600 {
        w.step(&if tick % 24 == 0 {
            vec![Command::Fire(Shot::new(
                Ammunition::HighExplosive,
                Vector::ZERO,
            ))]
        } else {
            vec![]
        });
    }
    assert_eq!(w.broken, 0);
    assert_eq!(w.ablated, 0);
    assert_eq!(w.pieces.iter().filter(|p| p.active).count(), active);
    w.special = Some(crate::SpecialWeapon::Rocket);
    w.special_charges = 1;
    let shots = w.shots;
    w.step(&[Command::ActivateSpecial]);
    assert_eq!(w.shots, shots);
    assert_eq!(w.special_charges, 0);
}
#[test]
fn vehicle_speed_and_mass_are_distinct() {
    let speeds = VehicleKind::ALL.map(|kind| {
        let mut w = world(kind);
        // Compare cruising speeds after every class has finished accelerating.
        for _ in 0..900 {
            w.drive_tank(w.launcher, [1.0, 1.0], TICK_DT);
        }
        let body = &w.physics.bodies[w.pieces[w.launcher].body];
        assert!((body.mass() - kind.mass()).abs() < 1.0);
        body.linvel().x
    });
    assert!(speeds[0] > speeds[1] * 1.4, "{speeds:?}");
    assert!(speeds[2] < speeds[1], "{speeds:?}");
}

#[test]
fn mortar_round_crosses_cover_and_explodes_at_its_landing_point() {
    let mut w = world(VehicleKind::Mortar);
    let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
    w.physics.bodies[w.pieces[enemy].body]
        .set_position(Pose::new(Vector::new(2.0, 0.0), 0.0), true);
    w.add_box(
        Vector::new(-8.0, 0.0),
        Vector::new(0.5, 3.0),
        Material::Anchor,
        true,
    );
    w.physics
        .bodies
        .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
    w.fire(
        Shot::new(Ammunition::HighExplosive, Vector::new(2.0, 0.0)),
        &mut vec![],
    );
    assert_eq!(w.shells.len(), 1);
    let shell = w.shells[0];
    let landing = shell.position + shell.velocity * shell.flight;
    let mut events = vec![];
    for _ in 0..300 {
        events.extend(w.step(&[]));
    }
    assert!(w.shells.is_empty());
    assert!(w.tanks.iter().find(|t| t.id == enemy).unwrap().health < 100.0);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Event::Exploded{position,..} if position.distance(landing)<0.01))
    );
}

#[test]
fn sandbox_can_register_and_redeploy_each_vehicle_without_resetting_the_scene() {
    let mut w = World::new(Scene::Masonry);
    let pieces = w.pieces.len();
    for kind in VehicleKind::ALL {
        w.request_player_deployment();
        assert!(w.player_choosing_vehicle());
        w.step(&[Command::Deploy(kind)]);
        w.step(&[]);
        assert!(w.tank_alive());
        assert!(!w.player_choosing_vehicle());
        assert_eq!(w.player_vehicle(), kind);
        let body = &w.physics.bodies[w.pieces[w.launcher].body];
        assert!(body.linvel().is_finite() && body.angvel().is_finite());
        assert!(w.track_travel().iter().all(|distance| distance.is_finite()));
        assert_eq!(w.tanks.len(), 1);
        assert_eq!(w.pieces.len(), pieces);
    }
}

fn scout_range(speed: f32) -> World {
    let mut level = Level::default();
    level.size = [200.0, 200.0];
    level.items = vec![crate::level::LevelItem::Spawn {
        team: Team::Blue,
        position: [0.0, 0.0],
        angle: 0.0,
    }];
    let mut w = World::from_level(&level, Team::Blue).unwrap();
    w.apply_vehicle(w.launcher, VehicleKind::Scout);
    w.physics.bodies[w.pieces[w.launcher].body].set_linvel(Vector::new(speed, 0.0), true);
    w
}

#[test]
fn scout_sight_is_full_circle_independent_of_aim_and_still_occluded() {
    let mut w = scout_range(0.0);
    let initial = w.player_vision(0.0);
    for aim in [
        0.0,
        0.7,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::PI,
        6.2,
    ] {
        let sight = w.player_vision(aim);
        assert_eq!(
            sight.boundary, initial.boundary,
            "scout aim must not rotate sight"
        );
        for i in 0..32 {
            let direction = Vector::from_angle(i as f32 * std::f32::consts::TAU / 32.0);
            assert!(
                sight.contains(direction * 23.9),
                "full range in every direction"
            );
            assert!(!sight.contains(direction * 24.1), "range must stay limited");
        }
    }
    w.add_box(
        Vector::new(-6.0, 0.0),
        Vector::new(0.2, 3.0),
        Material::Anchor,
        true,
    );
    let sight = w.player_vision(0.0);
    assert!(
        !sight.contains(Vector::new(-10.0, 0.0)),
        "rear walls still block sight"
    );
    assert!(sight.contains(Vector::new(0.0, -12.0)));
    let replica = World::from_replica(replica::Map::capture(&w), replica::Frame::capture(&w));
    assert_eq!(replica.player_vehicle(), VehicleKind::Scout);
    assert_eq!(replica.player_vision(1.0).boundary, sight.boundary);
}

#[test]
fn scout_steering_cannot_pivot_at_rest() {
    for steering in [-1.0, 1.0] {
        let mut w = scout_range(0.0);
        for _ in 0..360 {
            w.step(&[Command::Tracks {
                left: steering * 0.5,
                right: -steering * 0.5,
            }]);
        }
        assert!(w.launcher_position().length() < 0.001);
        assert!(w.piece_pose(w.launcher).rotation.angle().abs() < 0.001);
    }
}

#[test]
fn scout_turns_preserve_cruising_speed_and_reverse_steering() {
    for direction in [-1.0, 1.0] {
        for steering in [-1.0, 1.0] {
            let mut w = scout_range(direction * VehicleKind::Scout.speed());
            for _ in 0..180 {
                w.step(&[Command::Tracks {
                    left: (direction + steering) * 0.5,
                    right: (direction - steering) * 0.5,
                }]);
                let body = &w.physics.bodies[w.pieces[w.launcher].body];
                assert!(
                    (9.7..10.3).contains(&body.linvel().length()),
                    "turn speed {}",
                    body.linvel().length()
                );
            }
            let body = &w.physics.bodies[w.pieces[w.launcher].body];
            assert!(body.angvel() * direction * steering > 1.0);
            assert!(
                w.launcher_position().length() > 8.0,
                "rolling turn, not a pivot"
            );
        }
    }
}

#[test]
fn scout_accelerates_through_turns_and_brakes_on_release() {
    let mut speeds = vec![];
    for steering in [0.0, 1.0] {
        let mut w = scout_range(0.0);
        for _ in 0..360 {
            w.step(&[Command::Tracks {
                left: (1.0 + steering) * 0.5,
                right: (1.0 - steering) * 0.5,
            }]);
        }
        speeds.push(
            w.physics.bodies[w.pieces[w.launcher].body]
                .linvel()
                .length(),
        );
        for _ in 0..360 {
            w.step(&[]);
        }
        let body = &w.physics.bodies[w.pieces[w.launcher].body];
        assert!(body.linvel().length() < 0.01);
        assert!(body.angvel().abs() < 0.01);
    }
    assert!((9.7..10.1).contains(&speeds[0]), "{speeds:?}");
    assert!(
        speeds[1] >= speeds[0] * 0.97,
        "steering must not reduce throttle: {speeds:?}"
    );
}

#[test]
fn scout_car_forces_still_respect_wall_collisions() {
    let mut w = scout_range(0.0);
    w.tanks[0].protection = 100.0;
    w.add_box(
        Vector::new(5.0, 0.0),
        Vector::new(0.5, 8.0),
        Material::Anchor,
        true,
    );
    for _ in 0..480 {
        w.step(&[Command::Tracks {
            left: 0.5,
            right: 0.5,
        }]);
    }
    assert!(w.launcher_position().x < 4.0);
    assert!(w.launcher_position().x > 2.0);
}

#[test]
fn scout_near_a_wall_keeps_momentum_until_physical_contact() {
    let mut samples = Vec::new();
    for obstacle in [false, true] {
        let mut w = scout_range(0.0);
        if obstacle {
            w.add_box(
                Vector::ZERO,
                Vector::new(0.25, 8.0),
                Material::Masonry,
                false,
            );
        }
        let body = w.pieces[w.launcher].body;
        w.physics.bodies[body].set_translation(Vector::new(-1.16, 0.0), true);
        w.physics.bodies[body].set_linvel(Vector::new(10.0, 3.0), true);
        w.physics.bodies[body].set_angvel(0.5, true);
        w.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
        w.drive_tank(w.launcher, [0.5, 0.5], TICK_DT / SUBSTEPS as f32);
        let car = &w.physics.bodies[body];
        samples.push((car.linvel(), car.angvel()));
    }
    assert_eq!(
        samples[0], samples[1],
        "a nearby wall must not cancel car velocity or spin"
    );
    assert!(samples[1].0.x > 9.0);
    assert!(samples[1].1 > 0.0);
}

#[test]
fn scout_collisions_transfer_momentum_but_do_not_bulldoze_walls() {
    use crate::level::{LevelItem, rectangle, wall_asset};
    for supported in [false, true] {
        let mut results = Vec::new();
        for kind in [VehicleKind::Scout, VehicleKind::Tank] {
            let mut level = Level {
                size: [80.0, 80.0],
                items: vec![
                    LevelItem::Spawn {
                        team: Team::Blue,
                        position: [-6.0, 0.0],
                        angle: 0.0,
                    },
                    LevelItem::Object {
                        asset: Box::new(wall_asset(Vector::new(0.5, 6.0))),
                        position: [0.0, 0.0],
                        angle: 0.0,
                        cutouts: vec![],
                    },
                ],
                ..Level::default()
            };
            if supported {
                for y in [-3.5, 3.5] {
                    level.items.push(LevelItem::Solid {
                        outline: rectangle(Vector::splat(0.5)),
                        position: [0.0, y],
                        angle: 0.0,
                        cutouts: vec![],
                    });
                }
            }
            let mut w = World::from_level(&level, Team::Blue).unwrap();
            w.apply_vehicle(w.launcher, kind);
            w.physics.bodies[w.pieces[w.launcher].body].set_linvel(Vector::X * kind.speed(), true);
            let wall: Vec<_> = w
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.object == Some(0))
                .map(|(id, _)| (id, w.piece_pose(id).translation))
                .collect();
            let mut impact = 0.0_f32;
            for _ in 0..720 {
                for event in w.step(&[Command::Tracks {
                    left: if kind == VehicleKind::Scout { 0.5 } else { 1.0 },
                    right: if kind == VehicleKind::Scout { 0.5 } else { 1.0 },
                }]) {
                    if let Event::Collided { energy, .. } = event {
                        impact = impact.max(energy);
                    }
                }
            }
            let moved = wall
                .iter()
                .filter(|&&(id, _)| w.pieces[id].active)
                .map(|&(id, p)| w.piece_pose(id).translation.distance(p))
                .fold(0.0_f32, f32::max);
            let health = w.tanks[0].health;
            results.push((impact, w.broken, moved, health, w.launcher_position().x));
        }
        assert!(results[0].0 > 80.0, "the scout must make a real impact");
        assert!(
            results[0].1 > 0 && results[0].2 > 0.001,
            "the wall must respond to a real physical collision: {results:?}"
        );
        assert!(
            results[0].3 < 99.0,
            "a hard ram must damage the scout: {results:?}"
        );
        assert!(
            results[0].4 < 0.5,
            "the scout must lose momentum at the wall: {results:?}"
        );
        if !supported {
            assert!(
                results[0].2 < results[1].2 * 0.5,
                "the light scout must displace the wall less than the tank: {results:?}"
            );
        } else {
            assert!(
                results[0].1 < results[1].1,
                "the scout must fracture fewer supported wall seams: {results:?}"
            );
        }
    }
}

#[test]
fn scout_sustained_fire_reliably_damages_and_destroys_a_tank() {
    let mut w = world(VehicleKind::Scout);
    let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
    let target = w.launcher_position() + Vector::X * 10.0;
    w.physics.bodies[w.pieces[enemy].body].set_translation(target, true);
    let mut killed = false;
    for tick in 0..480 {
        let events = w.step(&[
            Command::Aim(target),
            Command::Fire(Shot::new(Ammunition::ArmorPiercing, target)),
        ]);
        killed |= events.iter().any(|e| {
            matches!(
                e,
                Event::TankDestroyed {
                    team: Team::Red,
                    ..
                }
            )
        });
        if tick == 119 {
            assert!(
                w.tanks.iter().find(|t| t.id == enemy).unwrap().health <= 76.0,
                "scout must cause noticeable damage within one second"
            );
        }
        assert!(w.player_aim().spread <= 8.01_f32.to_radians());
        if killed {
            break;
        }
    }
    assert!(
        killed,
        "scout must defeat a tank with sustained accurate fire"
    );
}

#[test]
fn mortar_shell_fractures_rock_and_masonry_on_overhead_impact() {
    for preset in [crate::LibraryPreset::Rock, crate::LibraryPreset::Wall] {
        let mut w = World::from_object(&preset.asset()).unwrap();
        w.register_tank(w.launcher, Team::Blue, w.launcher_position(), 0.0);
        w.apply_vehicle(w.launcher, VehicleKind::Mortar);
        let initial_area: f32 = w
            .pieces
            .iter()
            .filter(|p| p.active && p.destructible())
            .map(|p| p.shape.area())
            .sum();
        let events = w.step(&[Command::Fire(Shot::new(
            Ammunition::HighExplosive,
            Vector::new(3.0, 0.0),
        ))]);
        assert!(events.iter().any(|e| matches!(e, Event::Fired { .. })));
        for _ in 0..360 {
            w.step(&[]);
        }
        assert_eq!(w.stats().explosions, 1);
        let largest = w
            .chunk_members()
            .values()
            .map(|ids| ids.iter().map(|&id| w.pieces[id].shape.area()).sum::<f32>())
            .fold(0.0_f32, f32::max);
        assert!(
            largest < initial_area * 0.9,
            "{preset:?}: shell must break material, largest {largest}/{initial_area}"
        );
        assert!(w.broken > 0);
    }
}

#[test]
fn heavy_mortar_drives_through_a_breakable_wall() {
    let mut w = World::from_object(&crate::LibraryPreset::Wall.asset()).unwrap();
    w.register_tank(w.launcher, Team::Blue, w.launcher_position(), 0.0);
    w.apply_vehicle(w.launcher, VehicleKind::Mortar);
    for _ in 0..960 {
        w.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
        if w.launcher_position().x > 5.0 {
            break;
        }
    }
    assert!(w.tank_alive(), "mortar must survive breaching a wall");
    assert!(
        w.launcher_position().x > 5.0,
        "stuck at {:?}, health {}, broken {}",
        w.launcher_position(),
        w.tanks[0].health,
        w.broken
    );
    assert!(w.broken > 0);
}

#[test]
fn mortar_accelerates_gradually_and_eases_into_cruising_speed() {
    let mut w = scout_range(0.0);
    w.apply_vehicle(w.launcher, VehicleKind::Mortar);
    let mut samples = vec![];
    for tick in 1..=720 {
        w.step(&[Command::Tracks {
            left: 1.0,
            right: 1.0,
        }]);
        if [30, 60, 120, 240, 480, 720].contains(&tick) {
            samples.push(w.physics.bodies[w.pieces[w.launcher].body].linvel().x);
        }
    }
    assert!(
        samples[0] > 0.1 && samples[0] < 0.4,
        "gentle start: {samples:?}"
    );
    assert!(
        samples[2] > 0.8 && samples[2] < 1.2,
        "one-second speed: {samples:?}"
    );
    assert!(
        samples.windows(2).all(|s| s[1] > s[0]),
        "smooth buildup: {samples:?}"
    );
    assert!(
        samples[4] > 3.0 && samples[4] < 3.55,
        "approach the limit gradually: {samples:?}"
    );
    assert!(
        samples[5] > 3.5 && samples[5] <= 3.61,
        "heavy vehicle cruising speed: {samples:?}"
    );
    assert!(w.track_travel().iter().all(|distance| *distance > 10.0));
    let view = w.tanks().find(|t| t.id == w.launcher).unwrap();
    assert_eq!(view.track_travel, w.track_travel());
}

#[test]
fn mortar_tracks_reverse_and_counter_rotate_with_the_hull() {
    for inputs in [[-1.0, -1.0], [1.0, -1.0]] {
        let mut w = scout_range(0.0);
        w.apply_vehicle(w.launcher, VehicleKind::Mortar);
        for _ in 0..240 {
            w.step(&[Command::Tracks {
                left: inputs[0],
                right: inputs[1],
            }]);
        }
        let travel = w.track_travel();
        for side in 0..2 {
            assert!(travel[side] * inputs[side] > 0.3, "{inputs:?}: {travel:?}");
        }
        let replica = World::from_replica(replica::Map::capture(&w), replica::Frame::capture(&w));
        let received = replica.tanks().find(|t| t.id == w.launcher).unwrap();
        assert_eq!(received.track_travel, travel);
    }
}

#[test]
fn mortar_projectiles_sample_the_shown_impact_disk_deterministically() {
    fn sample(speed: f32) -> Vec<Vector> {
        let mut w = scout_range(speed);
        w.apply_vehicle(w.launcher, VehicleKind::Mortar);
        let target = Vector::new(20.0, 0.0);
        for _ in 0..360 {
            w.update_player_aim(&[Command::Aim(target)]);
        }
        let spread = w.player_aim.spread;
        let radius = w.player_aim().mortar_impact_radius(20.0);
        let mut offsets = vec![];
        for _ in 0..64 {
            w.reload = 0.0;
            w.player_aim.spread = spread;
            w.fire(Shot::new(Ammunition::HighExplosive, target), &mut vec![]);
            let shell = w.shells.pop().unwrap();
            let offset = shell.position + shell.velocity * shell.flight - target;
            assert!(
                offset.length() <= radius + 0.0001,
                "landing outside shown circle"
            );
            offsets.push(offset);
        }
        assert!(offsets.iter().any(|o| o.x > radius * 0.5));
        assert!(offsets.iter().any(|o| o.x < -radius * 0.5));
        assert!(offsets.iter().any(|o| o.y > radius * 0.5));
        assert!(offsets.iter().any(|o| o.y < -radius * 0.5));
        offsets
    }
    let steady = sample(0.0);
    let moving = sample(3.6);
    assert_eq!(
        sample(3.6),
        moving,
        "same inputs reproduce the same landing points"
    );
    let average = |points: &[Vector]| {
        points.iter().map(|p| p.length_squared()).sum::<f32>() / points.len() as f32
    };
    assert!(average(&moving) > average(&steady) * 25.0);
}
