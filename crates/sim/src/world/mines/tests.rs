use super::*;
use crate::{
    SpecialWeapon,
    level::{Level, LevelItem},
};

fn world() -> World {
    let mut world = World::from_level(&Level::default(), Team::Blue).unwrap();
    world.enemy = None;
    world
}

fn move_tank(world: &mut World, id: usize, position: Vector) {
    world.physics.bodies[world.pieces[id].body].set_position(Pose::new(position, 0.0), true);
    world
        .physics
        .bodies
        .propagate_modified_body_positions_to_colliders(&mut world.physics.colliders);
    world.sight_cache = None;
}

fn mine(world: &mut World, team: Team) {
    world.mines.push(MineView {
        id: world.next_mine_id,
        position: Vector::ZERO,
        team,
        arming_remaining: 0.0,
    });
    world.next_mine_id += 1;
}

#[test]
fn pickup_roundtrip_placement_charges_and_independent_cooldowns() {
    let mut level = Level::default();
    level.items.push(LevelItem::Pickup {
        weapon: SpecialWeapon::Mine,
        position: [-18.0, 0.0],
        charges: 2,
        cooldown: 1.0,
        respawn_time: 20.0,
    });
    let loaded = Level::from_json(&level.to_json().unwrap()).unwrap();
    assert_eq!(level, loaded);
    let mut world = World::from_level(&loaded, Team::Blue).unwrap();
    world.step(&[]);
    assert_eq!(world.carried_special(), Some(SpecialWeapon::Mine));
    assert_eq!(world.special_charges(), 2);
    world.reload = 0.7;
    world.step(&[Command::ActivateSpecial]);
    assert_eq!(world.mines.len(), 1);
    assert_eq!(world.mines[0].position, world.launcher_position());
    assert_eq!(world.mines[0].team, Team::Blue);
    assert!(world.mines[0].arming_remaining > 0.7);
    assert_eq!(world.special_charges(), 1);
    assert!((world.reload - (0.7 - TICK_DT)).abs() < 0.001);
    world.step(&[Command::ActivateSpecial]);
    assert_eq!(world.mines.len(), 1, "cooldown blocks another placement");
    for _ in 0..125 {
        world.step(&[]);
    }
    let player = world.launcher;
    move_tank(&mut world, player, Vector::new(-10.0, 0.0));
    world.step(&[Command::ActivateSpecial]);
    assert_eq!(world.mines.len(), 2);
    assert_eq!(world.special_charges(), 0);
    assert_eq!(world.carried_special(), None);
    assert_eq!(world.shots, 0);
}

#[test]
fn invalid_placement_and_dead_vehicles_keep_no_deployable_mine() {
    let mut world = world();
    world.special = Some(SpecialWeapon::Mine);
    world.special_charges = 1;
    let player = world.launcher;
    move_tank(&mut world, player, Vector::new(30.0, 0.0));
    world.activate_special(&mut vec![]);
    assert_eq!(
        world.special_charges, 1,
        "invalid placement does not consume a charge"
    );
    assert!(world.mines.is_empty());
    world.destroy_tank(player, &mut vec![]);
    world.activate_special(&mut vec![]);
    assert!(world.mines.is_empty());
}

#[test]
fn arming_delay_friendly_safety_and_enemy_footprint_contact() {
    let mut world = world();
    let player = world.launcher;
    let enemy = world.tanks[1].id;
    move_tank(&mut world, player, Vector::ZERO);
    assert!(world.place_mine());
    world.advance_mines(ARM_SECONDS + 0.01, &mut vec![]);
    assert_eq!(world.mines.len(), 1, "placer cannot trigger an armed mine");
    move_tank(&mut world, player, Vector::new(-10.0, 0.0));
    move_tank(&mut world, enemy, Vector::new(1.5, 0.0));
    world.advance_mines(TICK_DT, &mut vec![]);
    assert_eq!(
        world.mines.len(),
        1,
        "nearby vehicle does not trigger without contact"
    );
    move_tank(&mut world, enemy, Vector::new(1.0, 0.0));
    world.mines[0].arming_remaining = 0.1;
    world.advance_mines(0.05, &mut vec![]);
    assert_eq!(world.mines.len(), 1, "no contact trigger before arming");
    let mut events = vec![];
    world.advance_mines(0.06, &mut events);
    assert!(world.mines.is_empty());
    assert!(!world.tank_is_alive(enemy));
    assert_eq!(world.tanks[0].health, 100.0);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Exploded { .. }))
            .count(),
        1
    );
}

#[test]
fn friendly_mines_add_exactly_one_meter_of_occluded_team_sight() {
    let mut world = world();
    mine(&mut world, Team::Blue);
    let vision = world.player_vision(std::f32::consts::PI);
    assert!(vision.contains(Vector::new(0.99, 0.0)));
    assert!(!vision.contains(Vector::new(1.01, 0.0)));
    world.add_box(
        Vector::new(0.5, 0.0),
        Vector::new(0.1, 2.0),
        Material::Anchor,
        true,
    );
    let vision = world.player_vision(std::f32::consts::PI);
    assert!(
        !vision.contains(Vector::new(0.8, 0.0)),
        "solid cover blocks mine sight"
    );
    assert!(vision.contains(Vector::new(-0.8, 0.0)));
    world.team = Team::Red;
    assert!(
        !world
            .player_vision(std::f32::consts::PI)
            .contains(Vector::ZERO),
        "enemy mine grants no sight"
    );
}

#[test]
fn enemy_detection_is_three_meters_shared_with_living_teammates_only() {
    let mut world = world();
    mine(&mut world, Team::Red);
    let player = world.launcher;
    move_tank(&mut world, player, Vector::new(-3.01, 0.0));
    assert_eq!(world.visible_mines().count(), 0);
    move_tank(&mut world, player, Vector::new(-3.0, 0.0));
    assert_eq!(world.visible_mines().count(), 1);
    move_tank(&mut world, player, Vector::new(-10.0, 0.0));
    assert_eq!(world.visible_mines().count(), 0, "detection has no memory");
    let ally = world.add_tank();
    world.register_tank(ally, Team::Blue, Vector::new(0.0, -2.9), 0.0);
    move_tank(&mut world, ally, Vector::new(0.0, -2.9));
    assert_eq!(world.visible_mines().count(), 1);
    world.destroy_tank(ally, &mut vec![]);
    assert_eq!(world.visible_mines().count(), 0);
    world.team = Team::Red;
    assert_eq!(
        world.visible_mines().count(),
        1,
        "owner sees mine from anywhere"
    );
}

#[test]
fn projectiles_destroy_hidden_mines_and_cover_intercepts_shots() {
    for ammunition in [
        Ammunition::HighExplosive,
        Ammunition::ArmorPiercing,
        Ammunition::Rocket,
    ] {
        for covered in [false, true] {
            let mut world = world();
            mine(&mut world, Team::Red);
            let player = world.launcher;
            move_tank(&mut world, player, Vector::new(-6.0, 0.0));
            if covered {
                world.add_box(
                    Vector::new(-2.0, 0.0),
                    Vector::new(0.2, 2.0),
                    Material::Anchor,
                    true,
                );
            }
            assert_eq!(world.visible_mines().count(), 0);
            world.fire(Shot::new(ammunition, Vector::new(6.0, 0.0)), &mut vec![]);
            assert_eq!(world.shots, 1);
            for _ in 0..120 {
                world.step(&[]);
            }
            assert_eq!(
                world.mines.len(),
                usize::from(covered),
                "{ammunition:?}, covered={covered}"
            );
        }
    }
}

#[test]
fn energy_weapons_destroy_mines_and_obey_cover() {
    for plasma in [false, true] {
        for covered in [false, true] {
            let mut world = world();
            mine(&mut world, Team::Red);
            let player = world.launcher;
            move_tank(&mut world, player, Vector::new(-6.0, 0.0));
            if covered {
                world.add_box(
                    Vector::new(-2.0, 0.0),
                    Vector::new(0.2, 2.0),
                    Material::Anchor,
                    true,
                );
            }
            if plasma {
                world.fire_plasma(Vector::new(-6.0, 0.0), Vector::X, &mut vec![]);
            } else {
                world.fire_laser(
                    Vector::new(-6.0, 0.0),
                    Vector::X,
                    Vector::X,
                    true,
                    &mut vec![],
                );
            }
            assert_eq!(world.mines.len(), usize::from(covered));
        }
    }
}

#[test]
fn scout_bullets_and_falling_mortar_rounds_destroy_mines() {
    for vehicle in [VehicleKind::Scout, VehicleKind::Mortar] {
        let mut world = world();
        mine(&mut world, Team::Red);
        let player = world.launcher;
        move_tank(&mut world, player, Vector::new(-10.0, 0.0));
        world.apply_vehicle(player, vehicle);
        world.fire(
            Shot::new(Ammunition::HighExplosive, Vector::ZERO),
            &mut vec![],
        );
        assert_eq!(world.shots, 1);
        for _ in 0..240 {
            world.step(&[]);
        }
        assert!(world.mines.is_empty(), "{vehicle:?} must destroy the mine");
    }
}

#[test]
fn deployed_mines_survive_owner_death_and_each_requires_a_living_target() {
    let mut world = world();
    mine(&mut world, Team::Blue);
    mine(&mut world, Team::Blue);
    let player = world.launcher;
    world.destroy_tank(player, &mut vec![]);
    assert_eq!(world.mines.len(), 2);
    let enemy = world.tanks[1].id;
    move_tank(&mut world, enemy, Vector::new(1.0, 0.0));
    world.advance_mines(TICK_DT, &mut vec![]);
    assert!(!world.tank_is_alive(enemy));
    assert_eq!(
        world.mines.len(),
        1,
        "the second mine cannot trigger on a dead vehicle"
    );
}

#[test]
fn mine_sight_reveals_an_enemy_in_grass_only_within_one_meter() {
    let mut world = world();
    world.terrain_mask = None;
    let enemy = world.tanks[1].id;
    world.terrain.push(crate::TerrainPatch {
        kind: crate::TerrainKind::TallGrass,
        min: Vector::new(0.5, -2.0),
        max: Vector::new(2.0, 2.0),
    });
    mine(&mut world, Team::Blue);
    move_tank(&mut world, enemy, Vector::new(0.9, 0.0));
    assert_eq!(
        world
            .player_vision(std::f32::consts::PI)
            .tank_visibility(enemy),
        Some(true)
    );
    move_tank(&mut world, enemy, Vector::new(1.1, 0.0));
    assert_eq!(
        world
            .player_vision(std::f32::consts::PI)
            .tank_visibility(enemy),
        Some(false)
    );
}

#[test]
fn replica_uses_filtered_mines_for_team_sight_and_live_detection() {
    let mut world = world();
    world.player_aim.angle = std::f32::consts::PI;
    mine(&mut world, Team::Blue);
    mine(&mut world, Team::Red);
    let frame = replica::Frame::capture(&world);
    assert_eq!(frame.mines.len(), 1);
    let client = World::from_replica(replica::Map::capture(&world), frame);
    assert_eq!(client.visible_mines().count(), 1);
    assert!(
        client
            .player_vision(std::f32::consts::PI)
            .contains(Vector::new(0.9, 0.0))
    );
    assert!(
        !client
            .player_vision(std::f32::consts::PI)
            .contains(Vector::new(1.1, 0.0))
    );
    let player = world.launcher;
    move_tank(&mut world, player, Vector::new(-2.9, 0.0));
    assert_eq!(replica::Frame::capture(&world).mines.len(), 2);
}

#[test]
fn bundled_maps_have_reachable_mine_pickups_for_both_teams() {
    for json in [
        include_str!("../../../../../levels/dota-level.json"),
        include_str!("../../../../../levels/classic-tanks-2.json"),
    ] {
        let level = Level::from_json(json).unwrap();
        let world = World::from_level(&level, Team::Blue).unwrap();
        let pads: Vec<_> = world
            .pickups()
            .filter(|p| p.weapon == SpecialWeapon::Mine)
            .collect();
        assert_eq!(pads.len(), 2);
        for pickup in pads {
            assert!(!world.terrain_at(pickup.position).blocks_tanks());
            assert!(
                world.tank_pose_clear(world.launcher, Pose::new(pickup.position, 0.0)),
                "mine pickup obstructed: {} {:?}",
                level.name,
                pickup.position
            );
        }
    }
}
