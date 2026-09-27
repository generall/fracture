//! Tank-only gameplay damage. Destructible scenery retains its material solver.
use super::*;
use crate::level::Team;

pub(super) const RESPAWN_SECONDS: f32 = 3.0;

pub(super) struct Tank {
    pub id: usize,
    pub team: Team,
    pub spawn: Pose,
    pub health: f32,
    pub respawn: Option<f32>,
    pub protection: f32,
    pub aim: aiming::AimState,
    pub reload: f32,
    pub track_travel: [f32; 2],
    pub reveal_until: u64,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TankView {
    pub id: usize,
    pub team: Team,
    pub health: f32,
    pub respawn_remaining: Option<f32>,
    pub protected: bool,
    pub turret_angle: f32,
    pub previous_turret_angle: f32,
    pub aim_spread: f32,
    pub reload: f32,
    pub track_travel: [f32; 2],
}

impl World {
    pub fn tanks(&self) -> impl Iterator<Item = TankView> + '_ {
        self.tanks
            .iter()
            .map(|t| TankView {
                id: t.id,
                team: t.team,
                health: t.health,
                respawn_remaining: t.respawn,
                protected: t.protection > 0.0,
                turret_angle: t.aim.angle,
                previous_turret_angle: t.aim.previous_angle,
                aim_spread: t.aim.spread,
                reload: if t.id == self.launcher {
                    self.reload
                } else {
                    t.reload
                },
                track_travel: t.track_travel,
            })
            .chain(self.replica.iter().flat_map(|f| f.tanks.iter().copied()))
    }
    pub fn player_tank_id(&self) -> usize {
        self.launcher
    }
    pub fn player_respawn_remaining(&self) -> Option<f32> {
        if let Some(frame) = &self.replica {
            return frame.respawn;
        }
        self.tanks
            .iter()
            .find(|t| t.id == self.launcher)
            .and_then(|t| t.respawn)
    }
    pub(super) fn tank_is_alive(&self, id: usize) -> bool {
        if id == self.launcher && self.tank_fall.is_some() {
            return false;
        }
        self.tanks.iter().find(|t| t.id == id).map_or_else(
            || id != self.launcher || self.tank_fall.is_none(),
            |t| t.health > 0.0,
        )
    }
    pub(super) fn register_tank(&mut self, id: usize, team: Team, position: Vector, angle: f32) {
        let spawn = Pose::new(position, angle);
        self.physics.bodies[self.pieces[id].body].set_position(spawn, true);
        self.pieces[id].previous_position = position;
        self.pieces[id].previous_angle = angle;
        if id == self.launcher {
            self.player_aim = aiming::AimState::new(angle);
        }
        self.tanks.push(Tank {
            id,
            team,
            spawn,
            health: 100.0,
            respawn: None,
            protection: 0.0,
            aim: aiming::AimState::new(angle),
            reload: 0.0,
            track_travel: [0.0; 2],
            reveal_until: 0,
        });
    }

    /// Armor is selected from the incoming direction in hull coordinates.
    /// Effective resistance: front 100 kJ, side 55 kJ, rear 30 kJ.
    pub(super) fn hit_tank(
        &mut self,
        id: usize,
        incoming: Vector,
        point: Vector,
        energy: f32,
        ammunition: Ammunition,
        events: &mut Vec<Event>,
    ) {
        let Some(tank) = self.tanks.iter().find(|t| t.id == id) else {
            return;
        };
        if tank.health <= 0.0 || tank.protection > 0.0 {
            return;
        }
        let forward = self.piece_pose(id).rotation * Vector::X;
        let facing = -incoming.normalize_or_zero().dot(forward);
        let armor = if facing > 0.5 {
            100_000.0
        } else if facing < -0.5 {
            30_000.0
        } else {
            55_000.0
        };
        let penetration = match ammunition {
            Ammunition::ArmorPiercing => 1.0,
            Ammunition::HighExplosive => 0.65,
            Ammunition::Rocket => 0.5,
            Ammunition::Laser | Ammunition::Plasma => 1.0,
        };
        let damage = ((energy * penetration - armor).max(0.0) / 6000.0).min(85.0);
        self.damage_tank_from(
            id,
            damage,
            TankImpact {
                cause: DestructionCause::Shell(ammunition),
                point,
                direction: incoming.normalize_or_zero(),
                impulse: incoming.normalize_or_zero()
                    * (2.0 * energy / incoming.length().max(0.001)),
            },
            events,
        );
    }
    #[cfg(test)]
    pub(super) fn damage_tank(&mut self, id: usize, damage: f32, events: &mut Vec<Event>) {
        self.damage_tank_from(
            id,
            damage,
            TankImpact {
                cause: DestructionCause::Blast,
                point: self.piece_pose(id).translation,
                direction: Vector::X,
                impulse: Vector::ZERO,
            },
            events,
        );
    }
    pub(super) fn damage_tank_from(
        &mut self,
        id: usize,
        damage: f32,
        impact: TankImpact,
        events: &mut Vec<Event>,
    ) {
        let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if tank.health <= 0.0 || tank.protection > 0.0 || damage <= 0.0 {
            return;
        }
        tank.health = (tank.health - damage).max(0.0);
        if tank.health == 0.0 {
            self.destroy_tank_from(id, impact, events);
        }
    }
    pub(super) fn destroy_tank(&mut self, id: usize, events: &mut Vec<Event>) {
        self.destroy_tank_from(
            id,
            TankImpact {
                cause: DestructionCause::Collapse,
                point: self.piece_pose(id).translation,
                direction: Vector::ZERO,
                impulse: Vector::ZERO,
            },
            events,
        );
    }
    fn destroy_tank_from(&mut self, id: usize, impact: TankImpact, events: &mut Vec<Event>) {
        let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if tank.respawn.is_some() {
            return;
        }
        tank.health = 0.0;
        tank.reveal_until = 0;
        tank.respawn = Some(RESPAWN_SECONDS);
        let body = &mut self.physics.bodies[self.pieces[id].body];
        let position = body.translation();
        let mass = body.mass();
        let omega = body.angvel();
        let team = tank.team;
        let details = TankDestruction {
            id,
            hull_angle: body.rotation().angle(),
            turret_angle: tank.aim.angle,
            velocity: body.linvel(),
            impact,
        };
        body.set_linvel(Vector::ZERO, false);
        body.set_angvel(0.0, false);
        body.set_enabled(false);
        if id == self.launcher {
            self.laser_beam = None;
            self.laser_remaining = 0.0;
            self.plasma_remaining = 0.0;
            self.plasma_beam = None;
            self.special = None;
            self.special_charges = 0;
            self.special_reload = 0.0;
        }
        events.push(Event::TankDestroyed {
            position,
            team,
            details,
        });
        self.spawn_tank_wreck(position, team, details, mass, omega);
    }
    pub(super) fn update_tanks(&mut self, events: &mut Vec<Event>) {
        if self.tanks.is_empty() {
            return;
        }
        for i in 0..self.tanks.len() {
            self.tanks[i].reload = (self.tanks[i].reload - TICK_DT).max(0.0);
            self.tanks[i].protection = (self.tanks[i].protection - TICK_DT).max(0.0);
            let Some(remaining) = self.tanks[i].respawn else {
                continue;
            };
            self.tanks[i].respawn = Some((remaining - TICK_DT).max(0.0));
            if remaining > TICK_DT {
                continue;
            }
            let id = self.tanks[i].id;
            let spawn = self.tanks[i].spawn;
            // Prefer the exact spawn; use a nearby clear footprint if occupied.
            let candidate = (0..49).find_map(|n| {
                let position = if n == 0 {
                    spawn.translation
                } else {
                    let ring = ((n - 1) / 16 + 1) as f32 * 2.0;
                    let angle = (n - 1) as f32 * std::f32::consts::TAU / 16.0;
                    spawn.translation + Vector::from_angle(angle) * ring
                };
                let pose = Pose::new(position, spawn.rotation.angle());
                self.tank_pose_clear(id, pose).then_some(pose)
            });
            let Some(pose) = candidate else {
                continue;
            };
            let tank = &mut self.tanks[i];
            tank.health = 100.0;
            tank.respawn = None;
            tank.protection = 1.0;
            tank.reload = 0.5;
            tank.aim = aiming::AimState::new(pose.rotation.angle());
            tank.track_travel = [0.0; 2];
            let body = &mut self.physics.bodies[self.pieces[id].body];
            body.set_position(pose, true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(0.0, true);
            body.set_enabled(true);
            self.pieces[id].previous_position = pose.translation;
            self.pieces[id].previous_angle = pose.rotation.angle();
            if id == self.launcher {
                self.player_aim = tank.aim;
                self.tank_fall = None;
                self.reload = 0.5;
                self.track_travel = [0.0; 2];
            } else if let Some(enemy) = &mut self.enemy {
                enemy.reset(pose.translation);
            }
            events.push(Event::TankRespawned {
                position: pose.translation,
                team: tank.team,
            });
        }
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
    }
    pub(super) fn tank_pose_clear(&self, id: usize, pose: Pose) -> bool {
        let body = &self.physics.bodies[self.pieces[id].body];
        let shapes: Vec<_> = body
            .colliders()
            .iter()
            .map(|&handle| {
                let own = &self.physics.colliders[handle];
                let local = body.position().inverse() * *own.position();
                let at = pose * local;
                let bounds = own.shape().compute_aabb(&at);
                (own, at, bounds)
            })
            .collect();
        let mut tank_min = Vector::splat(f32::INFINITY);
        let mut tank_max = Vector::splat(f32::NEG_INFINITY);
        for (_, _, bounds) in &shapes {
            if bounds
                .mins
                .cmplt(-self.arena_size * 0.5 + Vector::splat(0.16))
                .any()
                || bounds
                    .maxs
                    .cmpgt(self.arena_size * 0.5 - Vector::splat(0.16))
                    .any()
            {
                return false;
            }
            tank_min = tank_min.min(bounds.mins);
            tank_max = tank_max.max(bounds.maxs);
        }
        tank_min -= Vector::splat(0.0001);
        tank_max += Vector::splat(0.0001);
        // Compute each obstacle's bounds once for the complete tank, then run
        // precise hull/track overlap tests only on nearby compatible shapes.
        for (_, other) in self.physics.colliders.iter() {
            if other.parent() == Some(self.pieces[id].body)
                || !other.is_enabled()
                || other
                    .parent()
                    .is_some_and(|h| !self.physics.bodies[h].is_enabled())
                || !shapes
                    .iter()
                    .any(|(own, _, _)| own.collision_groups().test(other.collision_groups()))
            {
                continue;
            }
            let other_bounds = other.compute_aabb();
            if tank_min.cmpgt(other_bounds.maxs).any() || tank_max.cmplt(other_bounds.mins).any() {
                continue;
            }
            for (own, at, bounds) in &shapes {
                let bounds = bounds.loosened(0.0001);
                if !own.collision_groups().test(other.collision_groups())
                    || bounds.mins.cmpgt(other_bounds.maxs).any()
                    || bounds.maxs.cmplt(other_bounds.mins).any()
                {
                    continue;
                }
                if rapier2d::parry::query::intersection_test(
                    at,
                    own.shape(),
                    other.position(),
                    other.shape(),
                )
                .unwrap_or(true)
                {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{EnemyActivity, Level, LevelItem, rectangle};
    #[test]
    fn bounded_clearance_matches_exact_queries_for_rotated_tanks() {
        let world = World::new(Scene::ObjectYard);
        let id = world.launcher;
        let body = &world.physics.bodies[world.pieces[id].body];
        let exact = |pose: Pose| {
            body.colliders().iter().all(|&h| {
                let own = &world.physics.colliders[h];
                let at = pose * (body.position().inverse() * *own.position());
                let bounds = own.shape().compute_aabb(&at);
                if bounds
                    .mins
                    .cmplt(-world.arena_size * 0.5 + Vector::splat(0.16))
                    .any()
                    || bounds
                        .maxs
                        .cmpgt(world.arena_size * 0.5 - Vector::splat(0.16))
                        .any()
                {
                    return false;
                }
                world.physics.colliders.iter().all(|(_, other)| {
                    other.parent() == Some(world.pieces[id].body)
                        || !other.is_enabled()
                        || other
                            .parent()
                            .is_some_and(|h| !world.physics.bodies[h].is_enabled())
                        || !own.collision_groups().test(other.collision_groups())
                        || !rapier2d::parry::query::intersection_test(
                            &at,
                            own.shape(),
                            other.position(),
                            other.shape(),
                        )
                        .unwrap_or(true)
                })
            })
        };
        let mut blocked = 0;
        let mut clear = 0;
        for x in -12..=12 {
            for y in [-7.4, -3.3, 0.0, 3.5, 7.4] {
                for angle in [0.0, 0.3, 1.2, 2.7] {
                    let pose = Pose::new(Vector::new(x as f32 + 0.03, y), angle);
                    let reference = exact(pose);
                    assert_eq!(world.tank_pose_clear(id, pose), reference);
                    if reference {
                        clear += 1;
                    } else {
                        blocked += 1;
                    }
                }
            }
        }
        assert!(blocked > 0 && clear > 0);
    }
    #[test]
    fn destruction_event_preserves_shell_direction_pose_and_single_death() {
        for direction in [Vector::X, Vector::Y, -Vector::X, -Vector::Y] {
            let mut world = World::from_level(&level(EnemyActivity::Static), Team::Blue).unwrap();
            let id = world.tanks[1].id;
            world.tanks[1].health = 1.0;
            world.tanks[1].aim.angle = 0.7;
            let position = world.piece_pose(id).translation;
            let point = position - direction * 0.8;
            let mut events = vec![];
            world.hit_tank(
                id,
                direction * 500.0,
                point,
                1_000_000.0,
                Ammunition::ArmorPiercing,
                &mut events,
            );
            world.hit_tank(
                id,
                direction * 500.0,
                point,
                1_000_000.0,
                Ammunition::ArmorPiercing,
                &mut events,
            );
            let deaths: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::TankDestroyed { details, .. } => Some(details),
                    _ => None,
                })
                .collect();
            assert_eq!(deaths.len(), 1);
            assert_eq!(
                deaths[0].impact.cause,
                DestructionCause::Shell(Ammunition::ArmorPiercing)
            );
            assert_eq!(deaths[0].impact.point, point);
            assert_eq!(deaths[0].impact.direction, direction);
            assert_eq!(deaths[0].turret_angle, 0.7);
            assert!((deaths[0].hull_angle.abs() - std::f32::consts::PI).abs() < 0.001);
        }
    }
    #[test]
    fn destruction_from_ramming_has_contact_direction_without_explosion() {
        let mut map = level(EnemyActivity::Static);
        if let LevelItem::Spawn { position, .. } = &mut map.items[0] {
            *position = [-1.6, 0.0];
        }
        map.items.push(LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vector::new(0.5, 3.0)),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        let mut world = World::from_level(&map, Team::Blue).unwrap();
        world.tanks[0].health = 0.1;
        world.physics.bodies[world.pieces[world.launcher].body].set_linvel(Vector::X * 6.5, true);
        let mut events = vec![];
        for _ in 0..30 {
            events.extend(world.step(&[]));
        }
        let details = events
            .iter()
            .find_map(|e| match e {
                Event::TankDestroyed { details, .. } => Some(details),
                _ => None,
            })
            .expect("ram destroys weakened tank");
        assert_eq!(details.impact.cause, DestructionCause::Ram);
        assert!(
            details.impact.direction.x < -0.9,
            "force comes from the wall"
        );
        assert!((details.impact.point.x + 0.5).abs() < 0.1);
        assert!(!events.iter().any(|e| matches!(e, Event::Exploded { .. })));
    }
    fn level(activity: EnemyActivity) -> Level {
        Level {
            size: [32.0, 24.0],
            enemy_activity: activity,
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-10.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Spawn {
                    team: Team::Red,
                    position: [10.0, 0.0],
                    angle: std::f32::consts::PI,
                },
                LevelItem::Flag {
                    team: Team::Blue,
                    position: [-13.0, 0.0],
                },
                LevelItem::Flag {
                    team: Team::Red,
                    position: [13.0, 0.0],
                },
            ],
            ..Level::default()
        }
    }
    #[test]
    fn activities_move_fire_and_capture_as_requested() {
        for activity in EnemyActivity::ALL {
            let mut world = World::from_level(&level(activity), Team::Blue).unwrap();
            let id = world.enemy.as_ref().unwrap().id;
            let start = world.piece_pose(id).translation;
            let mut destroyed = 0;
            for _ in 0..if activity == EnemyActivity::Conquer {
                9600
            } else {
                2400
            } {
                for event in world.step(&[]) {
                    destroyed += usize::from(matches!(event, Event::TankDestroyed { .. }));
                }
                if world.winner.is_some() {
                    break;
                }
            }
            let moved = start.distance(world.piece_pose(id).translation);
            eprintln!(
                "{activity:?}: moved {moved}, shots {}, destroyed {destroyed}, winner {:?}, position {:?}",
                world.shots,
                world.winner,
                world.piece_pose(id).translation
            );
            match activity {
                EnemyActivity::Static => {
                    assert!(moved < 0.01);
                    assert_eq!(world.shots, 0);
                }
                EnemyActivity::Driving => {
                    assert!(moved > 3.0);
                    assert_eq!(world.shots, 0);
                }
                EnemyActivity::Attack => {
                    assert!(moved > 3.0);
                    assert!(world.shots > 0);
                    assert!(destroyed > 0);
                }
                EnemyActivity::Conquer => assert_eq!(world.winner, Some(Team::Red)),
            }
        }
    }
    #[test]
    fn armor_and_respawn_are_symmetric_and_owner_does_not_hide_the_player() {
        let mut world = World::from_level(&level(EnemyActivity::Static), Team::Blue).unwrap();
        let enemy = world.enemy.as_ref().unwrap().id;
        let mut damage = Vec::new();
        for incoming in [Vector::X, Vector::Y, -Vector::X] {
            world.tanks[1].health = 100.0;
            world.hit_tank(
                enemy,
                incoming,
                world.piece_pose(enemy).translation - incoming,
                490000.0,
                Ammunition::ArmorPiercing,
                &mut vec![],
            );
            damage.push(100.0 - world.tanks[1].health);
        }
        assert!(damage[0] < damage[1] && damage[1] < damage[2]);
        world.tanks[1].health = 100.0;
        world.hit_tank(
            enemy,
            Vector::X,
            world.piece_pose(enemy).translation - Vector::X,
            8670.0,
            Ammunition::Rocket,
            &mut vec![],
        );
        assert_eq!(world.tanks[1].health, 100.0);
        world.fire_from(
            enemy,
            Shot::new(Ammunition::ArmorPiercing, world.launcher_position()),
            &mut vec![],
        );
        for _ in 0..30 {
            world.step(&[]);
        }
        assert!(
            world.tanks[0].health < 100.0,
            "enemy shells must hit the player"
        );
        for id in [world.launcher, enemy] {
            world.destroy_tank(id, &mut vec![]);
        }
        for _ in 0..350 {
            world.step(&[]);
        }
        assert!(world.tanks.iter().all(|t| t.health == 0.0));
        for _ in 0..12 {
            world.step(&[]);
        }
        assert!(
            world
                .tanks
                .iter()
                .all(|t| t.health == 100.0 && t.protection > 0.0)
        );
        assert_eq!(world.tanks.len(), 2);
        assert_eq!(
            world
                .pieces
                .iter()
                .filter(|p| p.material == Material::Launcher)
                .count(),
            2
        );
        for id in [world.launcher, enemy] {
            world.damage_tank(id, 200.0, &mut vec![]);
        }
        assert!(world.tanks.iter().all(|t| t.health == 100.0));
    }
    #[test]
    fn routes_go_around_solid_and_massive_obstacles_and_allow_light_walls() {
        let mut map = level(EnemyActivity::Driving);
        map.items.push(LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vector::new(1.0, 5.0)),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        let world = World::from_level(&map, Team::Blue).unwrap();
        let route = world.tank_route(
            world.enemy.as_ref().unwrap().id,
            Vector::new(10.0, 0.0),
            Vector::new(-10.0, 0.0),
        );
        assert!(!route.is_empty());
        assert!(route.iter().any(|p| p.y.abs() > 6.0));
        assert!(route.iter().all(|p| p.x.abs() > 2.2 || p.y.abs() > 6.2));
        // A full-height wall closes all detours: light masonry is still a possible ram route.
        map.items.pop();
        map.items.push(LevelItem::Object {
            cutouts: vec![],
            asset: Box::new(crate::level::wall_asset(Vector::new(0.3, 22.0))),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        if let LevelItem::Object { asset, .. } = map.items.last_mut().unwrap() {
            asset.thickness = 0.2;
        }
        let world = World::from_level(&map, Team::Blue).unwrap();
        assert!(
            !world
                .tank_route(
                    world.enemy.as_ref().unwrap().id,
                    Vector::new(10.0, 0.0),
                    Vector::new(-10.0, 0.0)
                )
                .is_empty()
        );
        if let LevelItem::Object { asset, .. } = map.items.last_mut().unwrap() {
            asset.density = 10000.0;
        }
        let world = World::from_level(&map, Team::Blue).unwrap();
        assert!(
            world
                .tank_route(
                    world.enemy.as_ref().unwrap().id,
                    Vector::new(10.0, 0.0),
                    Vector::new(-10.0, 0.0)
                )
                .is_empty()
        );
    }
    #[test]
    fn conquer_drives_around_cover_and_cannot_shoot_through_it() {
        let mut map = level(EnemyActivity::Conquer);
        map.items.push(LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vector::new(1.0, 5.0)),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        let mut world = World::from_level(&map, Team::Blue).unwrap();
        let id = world.enemy.as_ref().unwrap().id;
        assert!(!world.tank_line_of_sight(id, world.launcher_position()));
        let mut max_y = 0.0_f32;
        for _ in 0..12000 {
            world.step(&[]);
            max_y = max_y.max(world.piece_pose(id).translation.y.abs());
            if world.winner.is_some() {
                break;
            }
        }
        assert!(
            max_y > 6.0,
            "enemy did not follow its route around cover: {max_y}"
        );
        assert_eq!(
            world.winner,
            Some(Team::Red),
            "enemy stalled at {:?}",
            world.piece_pose(id).translation
        );
    }

    #[test]
    fn enemy_blasts_a_route_through_breakable_cover() {
        for activity in [EnemyActivity::Attack, EnemyActivity::Conquer] {
            let mut map = level(activity);
            map.items.push(LevelItem::Object {
                cutouts: vec![],
                asset: Box::new(crate::level::wall_asset(Vector::new(0.3, 22.0))),
                position: [0.0, 0.0],
                angle: 0.0,
            });
            let mut world = World::from_level(&map, Team::Blue).unwrap();
            let id = world.enemy.as_ref().unwrap().id;
            assert!(
                world
                    .tank_route(id, Vector::new(10.0, 0.0), Vector::new(-10.0, 0.0))
                    .is_empty()
            );
            let mut fired_at_cover = false;
            for _ in 0..7200 {
                let hidden = !world.tank_line_of_sight(id, world.launcher_position());
                let shots = world.shots;
                world.step(&[]);
                fired_at_cover |= hidden && world.shots > shots;
                if world.piece_pose(id).translation.x < -2.0 {
                    break;
                }
            }
            assert!(fired_at_cover, "{activity:?} never fired at cover");
            assert!(
                world.piece_pose(id).translation.x < -2.0,
                "{activity:?} stalled at {:?}, shots {}",
                world.piece_pose(id).translation,
                world.shots
            );
        }
        let mut map = level(EnemyActivity::Attack);
        map.items.push(LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vector::new(0.15, 11.0)),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        let mut world = World::from_level(&map, Team::Blue).unwrap();
        for _ in 0..1200 {
            world.step(&[]);
        }
        assert_eq!(
            world.shots, 0,
            "enemy should not shoot an unbreakable barrier"
        );
    }

    #[test]
    fn ramming_damages_both_tanks_but_gentle_contact_and_pushing_do_not() {
        for speed in [0.5, 6.5] {
            let mut map = level(EnemyActivity::Static);
            map.items[0] = LevelItem::Spawn {
                team: Team::Blue,
                position: [-1.1, 0.0],
                angle: 0.0,
            };
            map.items[1] = LevelItem::Spawn {
                team: Team::Red,
                position: [1.1, 0.0],
                angle: std::f32::consts::PI,
            };
            let mut world = World::from_level(&map, Team::Blue).unwrap();
            let body = world.pieces[world.launcher].body;
            world.physics.bodies[body].set_linvel(Vector::X * speed, true);
            for _ in 0..60 {
                world.step(&[]);
            }
            let health: Vec<_> = world.tanks.iter().map(|t| t.health).collect();
            if speed < 1.5 {
                assert_eq!(health, vec![100.0, 100.0]);
            } else {
                assert!(
                    health.iter().all(|&hp| hp < 99.0 && hp > 70.0),
                    "ram health {health:?}"
                );
            }
        }
        let mut map = level(EnemyActivity::Static);
        map.items[0] = LevelItem::Spawn {
            team: Team::Blue,
            position: [-1.6, 0.0],
            angle: 0.0,
        };
        map.items.push(LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vector::new(0.5, 3.0)),
            position: [0.0, 0.0],
            angle: 0.0,
        });
        let mut world = World::from_level(&map, Team::Blue).unwrap();
        world.physics.bodies[world.pieces[world.launcher].body].set_linvel(Vector::X * 6.5, true);
        for _ in 0..60 {
            world.step(&[]);
        }
        let after_impact = world.tanks[0].health;
        assert!(
            after_impact < 99.0 && after_impact > 70.0,
            "wall impact health {after_impact}"
        );
        for _ in 0..600 {
            world.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
        }
        assert!(
            (world.tanks[0].health - after_impact).abs() < 0.01,
            "continuous pushing damaged the hull"
        );
    }

    #[test]
    fn ram_damage_scales_with_mass_and_speed_squared() {
        let mut damage = Vec::new();
        for (mass, speed) in [(4500.0, 3.0), (4500.0, 6.0), (9000.0, 3.0)] {
            let mut map = level(EnemyActivity::Static);
            map.items[0] = LevelItem::Spawn {
                team: Team::Blue,
                position: [-1.59, 0.0],
                angle: 0.0,
            };
            map.items.push(LevelItem::Solid {
                cutouts: vec![],
                outline: rectangle(Vector::new(0.5, 3.0)),
                position: [0.0, 0.0],
                angle: 0.0,
            });
            let mut world = World::from_level(&map, Team::Blue).unwrap();
            world
                .configure_test_tank(mass, Vector::new(-1.59, 0.0), speed)
                .unwrap();
            for _ in 0..60 {
                world.step(&[]);
            }
            damage.push(100.0 - world.tanks[0].health);
        }
        assert!(damage[0] > 1.0, "impact must cause damage: {damage:?}");
        assert!(
            (damage[1] / damage[0] - 4.0).abs() < 0.25,
            "doubling speed should quadruple damage: {damage:?}"
        );
        assert!(
            (damage[2] / damage[0] - 2.0).abs() < 0.15,
            "doubling mass should double damage: {damage:?}"
        );
    }

    #[test]
    fn settings_roundtrip_and_old_maps_default_to_static() {
        let mut map = level(EnemyActivity::Attack);
        assert_eq!(
            Level::from_json(&map.to_json().unwrap())
                .unwrap()
                .enemy_activity,
            EnemyActivity::Attack
        );
        let mut json: serde_json::Value = serde_json::from_str(&map.to_json().unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("enemy_activity");
        assert_eq!(
            Level::from_json(&json.to_string()).unwrap().enemy_activity,
            EnemyActivity::Static
        );
        map.enemy_activity = EnemyActivity::Conquer;
        let world = World::from_level(&map, Team::Red).unwrap();
        assert_eq!(world.tanks[1].team, Team::Blue);
    }
}
