//! Fixed defensive tank guns with limited traverse and repairable ownership.
use super::*;
use crate::level::Team;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TurretView {
    pub position: Vector,
    pub facing: f32,
    pub arc: f32,
    pub max_health: f32,
    pub capturable: bool,
    pub capturing: Option<Team>,
    pub progress: f32,
    pub contested: bool,
}
impl TurretView {
    /// Repair console sits behind the base, outside its returning collider.
    pub fn repair_position(self) -> Vector {
        self.position - Vector::from_angle(self.facing) * 2.5
    }
}
#[derive(Clone, Copy)]
pub(super) struct Mount {
    pub view: TurretView,
    ticks: u32,
}
impl Mount {
    pub fn new(position: Vector, facing: f32, arc: f32, capturable: bool, max_health: f32) -> Self {
        Self {
            view: TurretView {
                position,
                facing,
                arc,
                max_health,
                capturable,
                capturing: None,
                progress: 0.0,
                contested: false,
            },
            ticks: 0,
        }
    }
}
fn delta(a: f32, b: f32) -> f32 {
    (a - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}
impl World {
    pub(super) fn update_turrets(&mut self, events: &mut Vec<Event>) {
        let mounts: Vec<_> = self
            .tanks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.mount.map(|m| (i, t.id, t.team, t.health, m)))
            .collect();
        for (index, id, team, health, mut mount) in mounts {
            if health <= 0.0 {
                let point = mount.view.repair_position();
                let present = |team| {
                    self.tanks.iter().any(|t| {
                        t.mount.is_none()
                            && t.team == team
                            && self.tank_is_alive(t.id)
                            && self.piece_pose(t.id).translation.distance_squared(point) <= 1.0
                    })
                };
                let blue = present(Team::Blue);
                let red = present(Team::Red);
                mount.view.contested = blue && red;
                if !mount.view.contested {
                    let capturing = if blue {
                        Some(Team::Blue)
                    } else if red {
                        Some(Team::Red)
                    } else {
                        None
                    };
                    let capturing = capturing.filter(|t| mount.view.capturable || *t == team);
                    if capturing != mount.view.capturing || capturing.is_none() {
                        mount.ticks = 0;
                    }
                    mount.view.capturing = capturing;
                    if capturing.is_some() {
                        mount.ticks = (mount.ticks + 1).min(1800);
                    }
                    mount.view.progress = mount.ticks as f32 / 1800.0;
                    if mount.ticks == 1800
                        && self
                            .tank_pose_clear(id, Pose::new(mount.view.position, mount.view.facing))
                    {
                        let t = &mut self.tanks[index];
                        t.team = capturing.unwrap();
                        t.health = mount.view.max_health;
                        t.respawn = None;
                        t.reload = 0.9;
                        t.aim = aiming::AimState::new(mount.view.facing);
                        self.physics.bodies[self.pieces[id].body].set_enabled(true);
                        self.query_epoch += 1;
                        mount.ticks = 0;
                        mount.view.progress = 0.0;
                        mount.view.capturing = None;
                        self.sight_cache = None;
                    }
                }
                self.tanks[index].mount = Some(mount);
                continue;
            }
            let current = self.tanks[index].aim.angle;
            let half = mount.view.arc.to_radians() * 0.5;
            let range = self.vision_range_at(mount.view.position);
            let target = self
                .tanks
                .iter()
                .filter(|t| t.id != id && t.team != team && self.tank_is_alive(t.id))
                .filter_map(|t| {
                    let p = self.piece_pose(t.id).translation;
                    let offset = p - mount.view.position;
                    let angle = offset.to_angle();
                    (offset.length() <= range
                        && delta(angle, mount.view.facing).abs() <= half
                        && (offset.length() <= NEAR_VISION
                            || delta(angle, current).abs() <= VISION_HALF_ANGLE)
                        && self.detects_tank(id, t.id))
                    .then_some((t.id, p, offset.length_squared()))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let desired = target.map_or_else(
                || mount.view.facing + half * (self.tick as f32 * TICK_DT * 0.35).sin(),
                |(_, p, _)| (p - mount.view.position).to_angle(),
            );
            self.advance_enemy_aim(id, desired);
            let aim = &mut self.tanks[index].aim;
            aim.angle = mount.view.facing + delta(aim.angle, mount.view.facing).clamp(-half, half);
            if let Some((target, p, _)) = target {
                let offset = p - mount.view.position;
                if delta(offset.to_angle(), aim.angle).abs() < 0.04
                    && self
                        .shell_hit(mount.view.position, offset, 1.0, 0.035, id)
                        .is_some_and(|(_, hit, _, _)| hit == target)
                {
                    self.fire_from(id, Shot::new(Ammunition::ArmorPiercing, p), events);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelItem};
    #[test]
    fn fixed_gun_withstands_a_full_speed_scout_ram_from_any_side() {
        for approach in [0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::PI] {
            let (mut w, gun) = fixture(false);
            let player = w.launcher;
            w.apply_vehicle(player, VehicleKind::Scout);
            let direction = Vector::from_angle(approach);
            park(&mut w, player, -direction * 3.0);
            w.physics.bodies[w.pieces[player].body].set_rotation(Rotation::new(approach), true);
            w.physics.bodies[w.pieces[player].body]
                .set_linvel(direction * VehicleKind::Scout.speed(), true);
            w.tanks.iter_mut().find(|t| t.id == gun).unwrap().reload = 1000.0;
            let mut events = vec![];
            for _ in 0..120 {
                events.extend(w.step(&[Command::Tracks {
                    left: 0.5,
                    right: 0.5,
                }]));
            }
            let gun_health = w.tanks.iter().find(|t| t.id == gun).unwrap().health;
            let scout_health = w.tanks.iter().find(|t| t.id == player).unwrap().health;
            assert!(
                gun_health >= 90.0,
                "fixed gun health {gun_health}, approach {approach}"
            );
            assert!(
                scout_health < 99.0,
                "the scout must bear the collision: {scout_health}"
            );
            assert_eq!(w.piece_pose(gun).translation, Vector::ZERO);
            assert!(
                !events.iter().any(
                    |e| matches!(e, Event::TankDestroyed { details, .. } if details.id == gun)
                )
            );
        }
    }
    fn fixture(capturable: bool) -> (World, usize) {
        configured(Team::Red, 100.0, capturable)
    }
    fn configured(team: Team, health: f32, capturable: bool) -> (World, usize) {
        let mut level = Level::default();
        level.items.push(LevelItem::Turret {
            team,
            position: [0.0, 0.0],
            angle: std::f32::consts::PI,
            arc: 120.0,
            health,
            capturable,
        });
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        w.enemy = None;
        let id = w.tanks.iter().find(|t| t.mount.is_some()).unwrap().id;
        (w, id)
    }
    fn park(w: &mut World, id: usize, p: Vector) {
        w.physics.bodies[w.pieces[id].body].set_position(Pose::new(p, 0.0), true);
        w.physics.bodies[w.pieces[id].body].set_linvel(Vector::ZERO, true);
        w.physics.bodies[w.pieces[id].body].set_angvel(0.0, true);
        w.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
    }
    #[test]
    fn gun_is_fixed_tracks_enemies_within_arc_and_respects_cover() {
        let (mut w, id) = fixture(true);
        let pose = w.piece_pose(id);
        for _ in 0..400 {
            w.step(&[]);
        }
        assert!(w.shots > 0);
        assert_eq!(w.piece_pose(id), pose);
        let t = w.tanks.iter().find(|t| t.id == id).unwrap();
        assert!(delta(t.aim.angle, std::f32::consts::PI).abs() <= 60.0_f32.to_radians());
        let (mut w, id) = fixture(true);
        let player = w.launcher;
        park(&mut w, player, Vector::new(12.0, 0.0));
        for _ in 0..300 {
            w.step(&[]);
        }
        assert_eq!(w.shots, 0, "enemy outside mount arc");
        park(&mut w, player, Vector::new(-12.0, 0.0));
        w.add_box(
            Vector::new(-6.0, 0.0),
            Vector::new(0.5, 4.0),
            Material::Anchor,
            true,
        );
        for _ in 0..300 {
            w.step(&[]);
        }
        assert_eq!(w.shots, 0, "cover blocks detection and firing");
        assert!(w.physics.bodies[w.pieces[id].body].is_fixed());
    }
    #[test]
    fn turret_keeps_tracking_a_rotated_scout_after_spawn_protection() {
        let (mut w, _) = fixture(true);
        w.require_vehicle_choice(w.launcher);
        w.step(&[Command::Deploy(VehicleKind::Scout)]);
        w.step(&[]);
        let player = w.launcher;
        park(&mut w, player, Vector::new(-10.0, 0.0));
        w.physics.bodies[w.pieces[player].body].set_rotation(Rotation::new(1.2), true);
        let mut events = vec![];
        for _ in 0..900 {
            events.extend(w.step(&[]));
        }
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::TankDestroyed {
                    team: Team::Blue,
                    ..
                }
            )),
            "shots {}",
            w.shots
        );
        assert!(!w.tank_alive());
    }
    #[test]
    fn destroy_then_capture_and_repair_changes_team_after_exactly_fifteen_seconds() {
        let (mut w, id) = fixture(true);
        let index = w.tanks.iter().position(|t| t.id == id).unwrap();
        let console = w.tanks[index].mount.unwrap().view.repair_position();
        let player = w.launcher;
        park(&mut w, player, console);
        for _ in 0..100 {
            w.update_turrets(&mut vec![]);
        }
        assert_eq!(w.tanks[index].team, Team::Red);
        assert_eq!(w.tanks[index].mount.unwrap().view.progress, 0.0);
        w.damage_tank(id, 1000.0, &mut vec![]);
        for _ in 0..1799 {
            w.step(&[]);
        }
        assert!(!w.tank_is_alive(id));
        assert_eq!(w.tanks[index].team, Team::Red);
        assert!(w.tanks[index].mount.unwrap().view.progress > 0.99);
        w.step(&[]);
        assert!(w.tank_is_alive(id));
        assert_eq!(w.tanks[index].team, Team::Blue);
        assert!(w.physics.bodies[w.pieces[id].body].is_fixed());
        let replica = World::from_replica(replica::Map::capture(&w), replica::Frame::capture(&w));
        let received = replica.tanks().find(|t| t.id == id).unwrap();
        assert_eq!(received.team, Team::Blue);
        assert!(received.mount.is_some());
    }
    #[test]
    fn capture_disabled_allows_only_owner_repair_and_leaving_resets() {
        let (mut w, id) = fixture(false);
        w.damage_tank(id, 1000.0, &mut vec![]);
        let index = w.tanks.iter().position(|t| t.id == id).unwrap();
        let console = w.tanks[index].mount.unwrap().view.repair_position();
        let player = w.launcher;
        park(&mut w, player, console);
        for _ in 0..1801 {
            w.step(&[]);
        }
        assert!(!w.tank_is_alive(id));
        assert_eq!(w.tanks[index].mount.unwrap().view.progress, 0.0);
        park(&mut w, player, Vector::new(-18.0, 0.0));
        let ally = w
            .tanks
            .iter()
            .find(|t| t.team == Team::Red && t.mount.is_none())
            .unwrap()
            .id;
        park(&mut w, ally, console);
        for _ in 0..600 {
            w.step(&[]);
        }
        assert!(w.tanks[index].mount.unwrap().view.progress > 0.3);
        park(&mut w, ally, Vector::new(18.0, 0.0));
        w.step(&[]);
        assert_eq!(w.tanks[index].mount.unwrap().view.progress, 0.0);
        park(&mut w, ally, console);
        for _ in 0..1800 {
            w.step(&[]);
        }
        assert!(w.tank_is_alive(id));
        assert_eq!(w.tanks[index].team, Team::Red);
    }
    #[test]
    fn neutral_guns_attack_both_teams_and_restore_custom_health_on_capture() {
        for team in [Team::Blue, Team::Red] {
            let (mut w, id) = configured(Team::Neutral, 750.0, true);
            let player = w.launcher;
            w.tanks.iter_mut().find(|t| t.id == player).unwrap().team = team;
            let index = w.tanks.iter().position(|t| t.id == id).unwrap();
            assert_eq!(w.tanks[index].health, 750.0);
            w.tanks
                .iter_mut()
                .find(|t| t.id == player)
                .unwrap()
                .protection = 100.0;
            for _ in 0..150 {
                w.step(&[]);
            }
            assert!(w.shots > 0, "neutral turret must fire at {team:?}");
            w.damage_tank(id, 200.0, &mut vec![]);
            assert_eq!(w.tanks[index].health, 550.0);
            w.damage_tank(id, 600.0, &mut vec![]);
            let console = w.tanks[index].mount.unwrap().view.repair_position();
            park(&mut w, player, console);
            for _ in 0..1800 {
                w.step(&[]);
            }
            assert_eq!(w.tanks[index].team, team);
            assert_eq!(w.tanks[index].health, 750.0);
            let replica =
                World::from_replica(replica::Map::capture(&w), replica::Frame::capture(&w));
            let received = replica.tanks().find(|t| t.id == id).unwrap();
            assert_eq!(received.health, 750.0);
            assert_eq!(received.mount.unwrap().max_health, 750.0);
        }
    }
    #[test]
    fn noncapturable_neutral_ruins_cannot_be_repaired_by_either_team() {
        for team in [Team::Blue, Team::Red] {
            let (mut w, id) = configured(Team::Neutral, 100.0, false);
            let player = w.launcher;
            w.tanks.iter_mut().find(|t| t.id == player).unwrap().team = team;
            w.damage_tank(id, 1000.0, &mut vec![]);
            let index = w.tanks.iter().position(|t| t.id == id).unwrap();
            let console = w.tanks[index].mount.unwrap().view.repair_position();
            park(&mut w, player, console);
            for _ in 0..1801 {
                w.update_turrets(&mut vec![]);
            }
            assert_eq!(w.tanks[index].team, Team::Neutral);
            assert_eq!(w.tanks[index].health, 0.0);
            assert_eq!(w.tanks[index].mount.unwrap().view.progress, 0.0);
        }
    }
    #[test]
    fn allied_turret_shares_live_sight_and_repaired_ownership_with_replicas() {
        let (mut w, id) = configured(Team::Blue, 300.0, true);
        let player = w.launcher;
        park(&mut w, player, Vector::new(18.0, -10.0));
        let index = w.tanks.iter().position(|t| t.id == id).unwrap();
        let target = Vector::new(-8.0, 0.0);
        let check = |w: &World, expected: bool| {
            assert_eq!(w.player_vision(0.0).contains(target), expected);
            let replica = World::from_replica(replica::Map::capture(w), replica::Frame::capture(w));
            assert_eq!(replica.player_vision(0.0).contains(target), expected);
        };
        check(&w, true);
        assert!(
            !w.player_vision(0.0).contains(Vector::new(8.0, 0.0)),
            "rear outside near circle"
        );
        w.tanks[index].team = Team::Neutral;
        check(&w, false);
        w.tanks[index].team = Team::Red;
        check(&w, false);
        w.damage_tank(id, 1000.0, &mut vec![]);
        check(&w, false);
        let console = w.tanks[index].mount.unwrap().view.repair_position();
        park(&mut w, player, console);
        for _ in 0..1800 {
            w.step(&[]);
        }
        park(&mut w, player, Vector::new(18.0, -10.0));
        check(&w, true);
        w.add_box(
            Vector::new(-4.0, 0.0),
            Vector::new(0.5, 4.0),
            Material::Anchor,
            true,
        );
        check(&w, false);
    }
    #[test]
    fn health_defaults_for_old_maps_and_rejects_invalid_values() {
        let mut level = Level::default();
        level.items.push(LevelItem::Turret {
            team: Team::Neutral,
            position: [0.0, 0.0],
            angle: 0.0,
            arc: 180.0,
            health: 250.0,
            capturable: true,
        });
        assert_eq!(Level::from_json(&level.to_json().unwrap()).unwrap(), level);
        let mut json = serde_json::to_value(&level).unwrap();
        json["items"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("health");
        let old = Level::from_json(&json.to_string()).unwrap();
        assert!(matches!(
            old.items.last(),
            Some(LevelItem::Turret { health: 100.0, .. })
        ));
        for bad in [0.0, -1.0, 100_001.0, f32::NAN, f32::INFINITY] {
            if let LevelItem::Turret { health, .. } = level.items.last_mut().unwrap() {
                *health = bad;
            }
            assert!(level.validate().is_err());
        }
    }
    #[test]
    fn turret_settings_round_trip_and_reject_invalid_arcs() {
        let mut level = Level::default();
        level.items.push(LevelItem::Turret {
            team: Team::Blue,
            position: [0.0, 0.0],
            angle: 0.7,
            arc: 90.0,
            health: 100.0,
            capturable: true,
        });
        assert_eq!(Level::from_json(&level.to_json().unwrap()).unwrap(), level);
        for bad in [0.0, 361.0, f32::NAN] {
            if let LevelItem::Turret { arc, .. } = level.items.last_mut().unwrap() {
                *arc = bad;
            }
            assert!(level.validate().is_err());
        }
    }
}
