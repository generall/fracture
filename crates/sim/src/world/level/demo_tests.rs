//! Exercise the authored map with real track commands; no navigation feature is
//! added to the game. Both teams must retain a route after the bridge is lost.
use super::*;

fn drive_route(world: &mut World, waypoints: &[Vector]) {
    let mut next = 0;
    for _ in 0..18_000 {
        if world.winner().is_some() {
            return;
        }
        assert!(
            world.tank_alive(),
            "Tank sank on route near {:?}",
            world.launcher_position()
        );
        let pose = world.piece_pose(world.launcher);
        let delta = waypoints[next] - pose.translation;
        if delta.length() < 0.35 && next + 1 < waypoints.len() {
            next += 1;
            continue;
        }
        let error = (delta.y.atan2(delta.x) - pose.rotation.angle() + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let forward = if error.abs() < 0.2 {
            (delta.length() * 0.6).min(0.85)
        } else {
            0.0
        };
        let turn = (error * 1.6).clamp(-0.8, 0.8);
        world.step(&[Command::Tracks {
            left: forward + turn,
            right: forward - turn,
        }]);
    }
    panic!(
        "Route stalled at waypoint {next}: {:?}",
        world.launcher_position()
    );
}

#[test]
fn demo_is_stable_at_rest_and_fits_a_small_map_budget() {
    let level = Level::demo();
    let mut world = World::from_level(&level, Team::Blue).unwrap();
    assert!(world.stats().cells < 1600, "{} cells", world.stats().cells);
    // An overlapping barrel/fence can remain constrained at rest. Check actual
    // colliders as well as later motion, ignoring cells of the same object.
    for (index, a) in world
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.object.is_some() && !p.bridge_deck)
    {
        for b in world
            .pieces
            .iter()
            .skip(index + 1)
            .filter(|p| p.object.is_some() && p.object != a.object && !p.bridge_deck)
        {
            let ac = &world.physics.colliders[a.collider];
            let bc = &world.physics.colliders[b.collider];
            assert!(
                !rapier2d::parry::query::intersection_test(
                    ac.position(),
                    ac.shape(),
                    bc.position(),
                    bc.shape()
                )
                .unwrap(),
                "Overlapping demo objects {:?} and {:?}",
                a.object,
                b.object
            );
        }
    }
    let originals: Vec<_> = world.bodies().map(|b| (b.id, b.position)).collect();
    for _ in 0..240 {
        world.step(&[]);
    }
    assert_eq!(
        world.stats().broken,
        0,
        "The map should not damage itself at startup"
    );
    assert_eq!(world.stats().explosions, 0);
    assert!(world.bridges().all(|b| b.intact));
    for body in world.bodies() {
        let initial = originals.iter().find(|(id, _)| *id == body.id).unwrap().1;
        assert!(
            body.position.distance(initial) < 0.025,
            "Overlapping object {} moved {} m",
            body.id,
            body.position.distance(initial)
        );
    }
}

#[test]
fn demo_central_bridge_route_reaches_both_enemy_flags() {
    for team in [Team::Blue, Team::Red] {
        let level = Level::demo();
        let mut world = World::from_level(&level, team).unwrap();
        let enemy = world.flags().find(|(t, _)| *t != team).unwrap().1;
        drive_route(&mut world, &[enemy]);
        assert_eq!(world.winner(), Some(team));
        assert!(world.bridges().all(|b| b.intact));
        assert_eq!(world.stats().broken, 0);
    }
}

#[test]
fn demo_has_two_drivable_bypasses_for_each_team_after_bridge_destruction() {
    for team in [Team::Blue, Team::Red] {
        for y in [-11.5, 11.5] {
            let mut world = World::from_level(&Level::demo(), team).unwrap();
            let control =
                world.bridges().next().unwrap().controls[if team == Team::Blue { 1 } else { 0 }];
            for _ in 0..16 {
                world.step(&[Command::Fire(Shot::new(Ammunition::HighExplosive, control))]);
                for _ in 0..70 {
                    world.step(&[]);
                }
                if world.bridges().all(|b| !b.intact) {
                    break;
                }
            }
            assert!(
                world.bridges().all(|b| !b.intact),
                "The booths must be reachable by fire from each spawn"
            );
            let sign = if team == Team::Blue { -1.0 } else { 1.0 };
            drive_route(
                &mut world,
                &[
                    Vector::new(sign * 16.0, 0.0),
                    Vector::new(sign * 16.0, y),
                    Vector::new(-sign * 16.0, y),
                    Vector::new(-sign * 16.0, 0.0),
                    Vector::new(-sign * 21.5, 0.0),
                ],
            );
            assert_eq!(world.winner(), Some(team), "{team:?} via {y}");
        }
    }
}
