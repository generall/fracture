//! The local opponent uses the same restricted controller and human input path
//! as arena bots. World state is selected only while capturing its own view.
use super::*;
use crate::{
    ai::{Config, Controller},
    level::EnemyActivity,
    multiplayer::PlayerInput,
};
mod navigation;

pub(super) struct Enemy {
    pub id: usize,
    activity: EnemyActivity,
    controller: Option<Controller>,
    input: PlayerInput,
    next_decision: u64,
    weapons: multiplayer::Weapons,
}
impl Enemy {
    pub fn new(id: usize, activity: EnemyActivity, _position: Vector) -> Self {
        Self {
            id,
            activity,
            controller: None,
            input: PlayerInput::default(),
            next_decision: 0,
            weapons: multiplayer::Weapons::default(),
        }
    }
    pub fn reset(&mut self, position: Vector) {
        *self = Self::new(self.id, self.activity, position);
    }
}
impl World {
    pub(super) fn enemy_commands(&mut self, events: &mut Vec<Event>) -> Option<(usize, [f32; 2])> {
        let mut enemy = self.enemy.take()?;
        let id = enemy.id;
        if !self.tank_is_alive(id) || enemy.activity == EnemyActivity::Static {
            self.enemy = Some(enemy);
            return Some((id, [0.0; 2]));
        }
        let saved = (
            self.launcher,
            self.team,
            self.player_aim,
            self.reload,
            self.track_travel,
            self.tank_fall,
            multiplayer::Weapons::capture(self),
        );
        let tank = self.tanks.iter().find(|t| t.id == id).unwrap();
        self.launcher = id;
        self.team = tank.team;
        self.player_aim = tank.aim;
        self.reload = tank.reload;
        self.track_travel = tank.track_travel;
        self.tank_fall = None;
        enemy.weapons.apply(self);
        if self.tick >= enemy.next_decision {
            let mut map = self.ai_map();
            if enemy.activity == EnemyActivity::Driving {
                // Public deterministic patrol points; no query of unseen physics.
                let angle = (self.tick / 1800 + 1) as f32 * 2.399963;
                map.flags = vec![(
                    self.team.other(),
                    Vector::from_angle(angle) * self.arena_size * 0.30,
                )];
            }
            let mut observation = profile!(self, ai_sight, { self.ai_observation() });
            if enemy.activity == EnemyActivity::Driving {
                observation.targets.clear();
            }
            let controller = enemy.controller.get_or_insert_with(|| {
                Controller::new(
                    Config {
                        advance_objective: enemy.activity == EnemyActivity::Conquer,
                        hunt: enemy.activity == EnemyActivity::Attack,
                        ..Default::default()
                    },
                    map,
                )
            });
            if enemy.activity == EnemyActivity::Driving {
                let angle = (self.tick / 1800 + 1) as f32 * 2.399963;
                controller.set_patrol_goal(Vector::from_angle(angle) * self.arena_size * 0.30);
            }
            enemy.input = profile!(self, ai_navigation, { controller.decide(&observation) });
            if enemy.activity == EnemyActivity::Driving {
                enemy.input.fire = false;
            }
            enemy.next_decision = self.tick + 12;
        }
        let tracks = self.control_player(&enemy.input.commands(), false, false, events);
        let tank = self.tanks.iter_mut().find(|t| t.id == id).unwrap();
        tank.aim = self.player_aim;
        tank.reload = self.reload;
        enemy.weapons = multiplayer::Weapons::capture(self);
        self.launcher = saved.0;
        self.team = saved.1;
        self.player_aim = saved.2;
        self.reload = saved.3;
        self.track_travel = saved.4;
        self.tank_fall = saved.5;
        saved.6.apply(self);
        self.enemy = Some(enemy);
        Some((id, tracks))
    }
    #[cfg(test)]
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
