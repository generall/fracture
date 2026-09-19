//! Deterministic opponent controller: route planning, differential tracks and aiming.
use super::*;
use crate::level::EnemyActivity;
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
    shot: usize,
    progress_at: Vector,
    stuck_for: f32,
    reverse_for: f32,
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
            shot: 0,
            progress_at: position,
            stuck_for: 0.0,
            reverse_for: 0.0,
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
            enemy.observed = self.launcher_position();
            enemy.observe_in = 0.35;
        }
        let distance = enemy.observed.distance(position);
        let hunts = matches!(
            enemy.activity,
            EnemyActivity::Attack | EnemyActivity::Conquer
        );
        let visible = hunts
            && self.tank_alive()
            && distance
                < if enemy.activity == EnemyActivity::Conquer {
                    12.0
                } else {
                    30.0
                }
            && self.tank_line_of_sight(id, enemy.observed);
        enemy.visible_for = if visible {
            enemy.visible_for + TICK_DT
        } else {
            0.0
        };
        let stop_to_fire = enemy.activity == EnemyActivity::Attack && visible && distance < 8.0;
        if enemy.plan_in <= 0.0 {
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
            }
        }
        while enemy
            .route
            .first()
            .is_some_and(|p| p.distance(position) < 0.75)
        {
            enemy.route.remove(0);
        }
        if !stop_to_fire && let Some(target) = enemy.route.first() {
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
        // Independently turn the turret; a new target must remain visible for
        // 0.7 s before firing. Samples are delayed and aim has deterministic error.
        let desired = if hunts {
            (enemy.observed - position).to_angle()
        } else {
            pose.rotation.angle()
        };
        let tank = self.tanks.iter_mut().find(|t| t.id == id).unwrap();
        let error = angle_difference(desired, tank.turret);
        tank.turret += error.clamp(-TICK_DT * 1.05, TICK_DT * 1.05);
        if visible && enemy.visible_for >= 0.7 && enemy.fire_in <= 0.0 && error.abs() < 0.06 {
            let moving = self.physics.bodies[self.pieces[id].body].linvel().length() > 0.5;
            let spread = if moving { 0.07 } else { 0.035 };
            let offset = ((enemy.shot as f32 + 1.0) * 2.399963).sin() * spread;
            let target = position + Vector::from_angle(tank.turret + offset) * distance;
            self.fire_from(id, Shot::new(Ammunition::ArmorPiercing, target), events);
            enemy.fire_in = 1.6;
            enemy.shot += 1;
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
