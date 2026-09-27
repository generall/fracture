//! Deterministic opponent controller: route planning, differential tracks and aiming.
use super::*;
use crate::level::EnemyActivity;
mod clearing;
mod navigation;

pub(super) struct Enemy {
    pub id: usize,
    activity: EnemyActivity,
    route: Vec<Vector>,
    goal: Vector,
    patrol: usize,
    plan_in: f32,
    observe_in: f32,
    observed: Vector,
    visible_for: f32,
    fire_in: f32,
    progress_at: Vector,
    stuck_for: f32,
    reverse_for: f32,
    scan_in: f32,
    obstruction: Option<(usize, Vector)>,
    obstruction_key: Option<usize>,
    obstruction_for: f32,
}
impl Enemy {
    pub fn new(id: usize, activity: EnemyActivity, position: Vector) -> Self {
        Self {
            id,
            activity,
            route: vec![],
            goal: position,
            patrol: 0,
            plan_in: 0.0,
            observe_in: 0.0,
            observed: position,
            visible_for: 0.0,
            fire_in: 0.8,
            progress_at: position,
            stuck_for: 0.0,
            reverse_for: 0.0,
            scan_in: 0.0,
            obstruction: None,
            obstruction_key: None,
            obstruction_for: 0.0,
        }
    }
    pub fn reset(&mut self, position: Vector) {
        *self = Self::new(self.id, self.activity, position);
    }
}
fn angle_difference(target: f32, current: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

impl World {
    pub(super) fn enemy_commands(&mut self, events: &mut Vec<Event>) -> Option<(usize, [f32; 2])> {
        let mut enemy = self.enemy.take()?;
        let id = enemy.id;
        let mut tracks = [0.0; 2];
        if !self.tank_is_alive(id) || enemy.activity == EnemyActivity::Static {
            self.enemy = Some(enemy);
            return Some((id, tracks));
        }
        let pose = self.piece_pose(id);
        let position = pose.translation;
        enemy.plan_in -= TICK_DT;
        enemy.observe_in -= TICK_DT;
        enemy.fire_in -= TICK_DT;
        if enemy.observe_in <= 0.0 {
            let foliage_involved = self.terrain_at(position) == crate::TerrainKind::TallGrass
                || self.terrain_at(self.launcher_position()) == crate::TerrainKind::TallGrass;
            if !foliage_involved || self.detects_tank(id, self.launcher) {
                enemy.observed = self.launcher_position();
            }
            enemy.observe_in = 0.35;
        }
        let distance = enemy.observed.distance(position);
        let hunts = matches!(
            enemy.activity,
            EnemyActivity::Attack | EnemyActivity::Conquer
        );
        let visible = profile!(self, ai_sight, {
            hunts
                && self.tank_alive()
                && distance
                    < if enemy.activity == EnemyActivity::Conquer {
                        12.0
                    } else {
                        self.vision_range_at(position).max(30.0)
                    }
                && self.detects_tank(id, self.launcher)
                && self.tank_line_of_sight(id, enemy.observed)
        });
        enemy.visible_for = if visible {
            enemy.visible_for + TICK_DT
        } else {
            0.0
        };
        let stop_to_fire = enemy.activity == EnemyActivity::Attack && visible && distance < 8.0;
        if enemy.plan_in <= 0.0 {
            profile!(self, ai_navigation, {
                enemy.plan_in = 1.0;
                enemy.goal = match enemy.activity {
                    EnemyActivity::Conquer => self
                        .flags
                        .iter()
                        .find(|f| f.team == self.team)
                        .map_or(enemy.observed, |f| f.position),
                    EnemyActivity::Attack => enemy.observed,
                    _ => enemy.goal,
                };
                if enemy.activity == EnemyActivity::Driving
                    && (enemy.route.is_empty() || position.distance(enemy.goal) < 1.5)
                {
                    // A reproducible patrol, selected from reachable points around the map.
                    for _ in 0..8 {
                        let direction = Vector::from_angle(enemy.patrol as f32 * 2.399963);
                        enemy.patrol += 1;
                        let target = direction * self.arena_size * 0.32;
                        let route = self.tank_route(id, position, target);
                        if !route.is_empty() && target.distance(position) > 4.0 {
                            enemy.goal = target;
                            enemy.route = route;
                            break;
                        }
                    }
                } else {
                    enemy.route = self.tank_route(id, position, enemy.goal);
                    if hunts
                        && (enemy.route.is_empty()
                            || enemy
                                .route
                                .last()
                                .is_some_and(|p| p.distance(enemy.goal) > 2.0))
                    {
                        enemy.route = self.plan_tank_route(id, position, enemy.goal, true);
                    }
                }
            });
        }
        while enemy
            .route
            .first()
            .is_some_and(|p| p.distance(position) < 0.75)
        {
            enemy.route.remove(0);
        }
        enemy.scan_in -= TICK_DT;
        if hunts && enemy.scan_in <= 0.0 {
            enemy.scan_in = 0.2;
            enemy.obstruction = profile!(self, ai_obstruction, {
                self.route_obstruction(id, &enemy.route)
            });
        }
        if enemy
            .obstruction
            .is_some_and(|(piece, _)| !self.pieces[piece].active)
        {
            enemy.obstruction = None;
            enemy.scan_in = 0.0;
            enemy.plan_in = 0.0;
        }
        // Hold the breach aim until the player is fully acquired. Switching
        // every tick at a ragged opening can keep a limited-speed turret
        // forever between the two targets.
        let clearing = if hunts && !(visible && enemy.visible_for >= 0.7) {
            enemy.obstruction
        } else {
            None
        };
        // A glimpse of the player through a ragged opening must not reset
        // preparation to breach the same obstacle that still blocks the hull.
        let key = enemy.obstruction.map(|(piece, _)| {
            self.pieces[piece]
                .object
                .unwrap_or(self.pieces[piece].origin)
        });
        enemy.obstruction_for = if key.is_some() && key == enemy.obstruction_key {
            enemy.obstruction_for + TICK_DT
        } else {
            0.0
        };
        enemy.obstruction_key = key;
        let clearance = clearing.map_or(f32::INFINITY, |(_, point)| point.distance(position));
        let stop_to_clear = clearing.is_some() && clearance < 4.5;
        if !stop_to_fire
            && !stop_to_clear
            && let Some(target) = enemy.route.first()
        {
            let delta = *target - position;
            let error = angle_difference(delta.to_angle(), pose.rotation.angle());
            // Large heading changes turn in place. Small changes bend the route.
            let turn = (error * 0.10).clamp(-0.10, 0.10);
            let forward = if error.abs() > 0.55 {
                0.0
            } else {
                0.52 * (1.0 - error.abs())
            };
            tracks = [forward + turn, forward - turn];
            enemy.stuck_for += TICK_DT;
            if position.distance(enemy.progress_at) > 0.35 {
                enemy.progress_at = position;
                enemy.stuck_for = 0.0;
            }
            if enemy.stuck_for > 4.0 {
                enemy.reverse_for = 1.2;
                enemy.stuck_for = 0.0;
                enemy.plan_in = 0.0;
            }
            if enemy.reverse_for > 0.0 {
                enemy.reverse_for -= TICK_DT;
                tracks = [-0.35, -0.28];
            }
        }
        if stop_to_clear {
            enemy.stuck_for = 0.0;
            enemy.reverse_for = 0.0;
            // Leave room for the barrel and HE blast before firing. Check the
            // retreat footprint so backing up never deliberately drives into cover.
            if clearance < 3.3 {
                let backwards = position - pose.rotation * Vector::X * 0.6;
                if profile!(self, ai_clearance, {
                    self.tank_pose_clear(id, Pose::new(backwards, pose.rotation.angle()))
                }) {
                    tracks = [-0.25; 2];
                }
            }
        }
        let aim = if let Some((_, point)) = clearing {
            point
        } else {
            enemy.observed
        };
        let target_distance = aim.distance(position);
        let desired = if hunts {
            (aim - position).to_angle()
        } else {
            pose.rotation.angle()
        };
        self.advance_enemy_aim(id, desired);
        let turret = self.tanks.iter().find(|t| t.id == id).unwrap().aim.angle;
        let error = angle_difference(desired, turret);
        let acquired = if clearing.is_some() {
            enemy.obstruction_for >= 0.7
        } else {
            visible && enemy.visible_for >= 0.7
        };
        if acquired && enemy.fire_in <= 0.0 && error.abs() < 0.06 && target_distance > 1.8 {
            let target = position + Vector::from_angle(turret) * target_distance;
            let ammunition = if clearing.is_some() && target_distance > 3.3 {
                Ammunition::HighExplosive
            } else {
                Ammunition::ArmorPiercing
            };
            let before = self.shots;
            self.fire_from(id, Shot::new(ammunition, target), events);
            if self.shots > before {
                enemy.fire_in = 1.6;
                enemy.plan_in = enemy.plan_in.min(0.25);
            } else {
                enemy.scan_in = 0.0;
                enemy.fire_in = 0.2;
            }
        }
        self.enemy = Some(enemy);
        Some((id, tracks))
    }

    pub(super) fn tank_line_of_sight(&self, id: usize, target: Vector) -> bool {
        let origin = self.piece_pose(id).translation;
        let direction = (target - origin).normalize_or_zero();
        let distance = origin.distance(target);
        let ray = Ray::new(origin, direction);
        let mut nearest = (distance, self.launcher);
        for (_, collider) in self.physics.colliders.iter() {
            let Some(piece) = self.weapon_target(collider) else {
                continue;
            };
            if piece == id {
                continue;
            }
            if let Some(t) = collider
                .shape()
                .cast_ray(collider.position(), &ray, distance, true)
                && t < nearest.0
            {
                nearest = (t, piece);
            }
        }
        nearest.1 == self.launcher
    }
}
