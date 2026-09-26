use super::*;
use crate::level::{Level, LevelItem, StationKind, StationSettings, Team};

fn world() -> World {
    World::from_level(&Level::default(), Team::Blue).unwrap()
}

fn debris(world: &mut World, x: f32, half: f32) -> usize {
    let id = world.add_box(
        Vector::new(x, 0.0),
        Vector::splat(half),
        Material::Panel,
        false,
    );
    world.pieces[id].debris_remaining = Some(3.0);
    id
}

fn active(world: &World) -> usize {
    world.pieces.iter().filter(|p| p.active).count()
}

fn selected(world: &World, excess: usize) -> BTreeSet<(u32, u32)> {
    let mut remove = BTreeSet::new();
    world.retire_excess_debris(active(world) - excess, &mut remove);
    remove
}

#[test]
fn budget_prefers_smallest_debris_then_farthest_and_ignores_retired_ids() {
    let mut world = world();
    let large = debris(&mut world, 400.0, 0.2);
    let near = debris(&mut world, 200.0, 0.1);
    let far = debris(&mut world, 300.0, 0.1);
    assert!(selected(&world, 0).is_empty());
    assert_eq!(
        selected(&world, 1),
        BTreeSet::from([world.pieces[far].body.into_raw_parts()])
    );
    assert_eq!(
        selected(&world, 2),
        BTreeSet::from([
            world.pieces[near].body.into_raw_parts(),
            world.pieces[far].body.into_raw_parts(),
        ])
    );
    world.pieces[large].active = false;
    let mut remove = BTreeSet::new();
    world.retire_excess_debris(active(&world), &mut remove);
    assert!(remove.is_empty(), "retired IDs consume no live budget");
}

#[test]
fn budget_protects_other_tanks_flags_radar_and_whole_footprints() {
    let level = Level {
        size: [200.0, 100.0],
        items: vec![
            LevelItem::Spawn {
                team: Team::Blue,
                position: [0.0, 0.0],
                angle: 0.0,
            },
            LevelItem::Spawn {
                team: Team::Red,
                position: [70.0, 0.0],
                angle: 0.0,
            },
            LevelItem::Flag {
                team: Team::Blue,
                position: [-70.0, 0.0],
            },
            LevelItem::Station {
                station: StationKind::Radar,
                position: [4.0, 0.0],
                settings: StationSettings {
                    capture_time: 0.1,
                    radar_radius: 80.0,
                    ..StationSettings::default()
                },
            },
        ],
        ..Level::default()
    };
    let mut world = World::from_level(&level, Team::Blue).unwrap();
    for _ in 0..13 {
        world.update_stations();
    }
    assert_eq!(world.stations().next().unwrap().owner, Some(Team::Blue));
    let own = debris(&mut world, 2.0, 0.1);
    let other = debris(&mut world, 72.0, 0.1);
    let flag = debris(&mut world, -72.0, 0.1);
    let radar = debris(&mut world, -45.0, 0.1);
    // Center is outside the other tank's protected radius, but an edge overlaps it.
    let edge = debris(&mut world, 104.0, 3.0);
    let far = debris(&mut world, 180.0, 0.1);
    let remove = selected(&world, 6);
    assert_eq!(
        remove,
        BTreeSet::from([world.pieces[far].body.into_raw_parts()])
    );
    for id in [own, other, flag, radar, edge] {
        assert!(!remove.contains(&world.pieces[id].body.into_raw_parts()));
    }
}

#[test]
fn budget_preserves_intact_and_fixed_objects_and_retires_whole_chunks() {
    let mut world = world();
    let intact = debris(&mut world, 150.0, 0.1);
    world.pieces[intact].debris_remaining = None;
    let fixed = debris(&mut world, 160.0, 0.1);
    world.pieces[fixed].material = Material::Anchor;
    world.physics.bodies[world.pieces[fixed].body].set_body_type(RigidBodyType::Fixed, false);
    let a = debris(&mut world, 200.0, 0.1);
    let b = debris(&mut world, 200.2, 0.1);
    world.connect(
        a,
        b,
        Vector::new(200.1, 0.0),
        Vector::X,
        0.2,
        Material::Panel,
    );
    world.rebuild_chunks();
    assert_eq!(world.pieces[a].body, world.pieces[b].body);
    assert_eq!(
        selected(&world, 1),
        BTreeSet::from([world.pieces[a].body.into_raw_parts()])
    );
    world.pieces[b].debris_remaining = None;
    assert!(
        selected(&world, 1).is_empty(),
        "a retained cell protects its whole chunk"
    );
}

#[test]
fn large_map_cleanup_removes_remote_debris_but_keeps_new_visible_wreck() {
    let mut world = world();
    while active(&world) < ACTIVE_PIECE_BUDGET {
        world.add_box(
            Vector::new(1000.0, 1000.0),
            Vector::splat(0.1),
            Material::Anchor,
            true,
        );
    }
    let old = debris(&mut world, 200.0, 0.1);
    let old_body = world.pieces[old].body;
    let old_collider = world.pieces[old].collider;
    let tank = world.launcher;
    world.destroy_tank(tank, &mut Vec::new());
    world.cleanup_debris();
    assert!(!world.pieces[old].active);
    assert!(world.physics.bodies.get(old_body).is_none());
    assert!(world.physics.colliders.get(old_collider).is_none());
    assert_eq!(
        world.bodies().filter(|b| b.tank_fragment.is_some()).count(),
        10
    );
    assert!(
        active(&world) > ACTIVE_PIECE_BUDGET,
        "visible wrecks may exceed the soft budget"
    );
}
