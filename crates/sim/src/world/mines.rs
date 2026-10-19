//! Nonblocking ground mines: team lookouts, proximity detection and weapon hits.
use super::*;
use crate::level::Team;
use rapier2d::parry::query::{ShapeCastOptions, cast_shapes};

#[cfg(test)]
mod tests;

pub(super) const MINE_RADIUS: f32 = 0.28;
pub(super) const MINE_SIGHT: f32 = 1.0;
const DETECTION_RANGE: f32 = 3.0;
const ARM_SECONDS: f32 = 0.75;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MineView {
    pub id: u64,
    pub position: Vector,
    pub team: Team,
    pub arming_remaining: f32,
}

impl World {
    pub fn mines(&self) -> impl Iterator<Item = MineView> + '_ {
        self.mines
            .iter()
            .chain(self.replica.iter().flat_map(|frame| frame.mines.iter()))
            .copied()
    }

    /// Mines never inherit radar, flag sight, or fog's remembered scenery.
    /// Any living allied vehicle can spot one for the team at close range.
    pub fn visible_mines(&self) -> impl Iterator<Item = MineView> + '_ {
        self.mines().filter(|mine| {
            self.replica.is_some()
                || mine.team == self.team
                || self.tanks.iter().any(|tank| {
                    tank.team == self.team
                        && tank.mount.is_none()
                        && self.tank_is_alive(tank.id)
                        && self
                            .piece_pose(tank.id)
                            .translation
                            .distance_squared(mine.position)
                            <= DETECTION_RANGE * DETECTION_RANGE
                        && self.sight_path_clear(
                            self.piece_pose(tank.id).translation,
                            Some(tank.id),
                            mine.position,
                            true,
                        )
                })
        })
    }

    pub(super) fn place_mine(&mut self) -> bool {
        let position = self.launcher_position();
        let half = self.arena_size * 0.5 - Vector::splat(MINE_RADIUS);
        if !position.is_finite()
            || position.abs().cmpgt(half).any()
            || self.terrain_at(position).blocks_tanks()
        {
            return false;
        }
        self.mines.push(MineView {
            id: self.next_mine_id,
            position,
            team: self.team,
            arming_remaining: ARM_SECONDS,
        });
        self.next_mine_id += 1;
        true
    }

    /// Exact footprint contact, checked after each physics microstep. Friendly
    /// vehicles pass safely, including the vehicle that placed the mine.
    pub(super) fn advance_mines(&mut self, dt: f32, events: &mut Vec<Event>) {
        for mine in &mut self.mines {
            mine.arming_remaining = (mine.arming_remaining - dt).max(0.0);
        }
        let triggered: Vec<_> = self
            .mines
            .iter()
            .filter_map(|mine| {
                if mine.arming_remaining > 1e-5 {
                    return None;
                }
                self.tanks
                    .iter()
                    .find(|tank| {
                        tank.team != mine.team
                            && tank.mount.is_none()
                            && self.tank_is_alive(tank.id)
                            && self.physics.bodies[self.pieces[tank.id].body]
                                .colliders()
                                .iter()
                                .any(|handle| {
                                    let collider = &self.physics.colliders[*handle];
                                    collider
                                        .shape()
                                        .project_point(collider.position(), mine.position, true)
                                        .point
                                        .distance_squared(mine.position)
                                        <= MINE_RADIUS * MINE_RADIUS
                                })
                    })
                    .map(|tank| (mine.id, tank.id))
            })
            .collect();
        for (mine, tank) in triggered {
            if !self.tank_is_alive(tank) {
                continue;
            }
            if let Some(index) = self.mines.iter().position(|m| m.id == mine) {
                let mine = self.mines.remove(index);
                // A direct undercarriage hit disables a full-health vehicle.
                self.damage_tank_from(
                    tank,
                    100.0,
                    TankImpact {
                        cause: DestructionCause::Blast,
                        point: mine.position,
                        direction: (self.piece_pose(tank).translation - mine.position)
                            .normalize_or_zero(),
                        impulse: Vector::ZERO,
                    },
                    events,
                );
                let event = self.next_impact();
                self.blast_pressure(mine.position, 300_000.0, 1.7, 0.0, event, events);
                self.explosions += 1;
                events.push(Event::Exploded {
                    position: mine.position,
                    radius: 1.7,
                });
                self.blast_decorations(mine.position, 1.7);
            }
        }
    }

    /// Sweep ground-level weapons separately from physical pieces, so mines
    /// cannot obstruct driving, navigation, or the shooter's own muzzle.
    pub(super) fn mine_hit(
        &self,
        origin: Vector,
        velocity: Vector,
        max_time: f32,
        radius: f32,
    ) -> Option<(f32, usize)> {
        self.mines
            .iter()
            .enumerate()
            .filter_map(|(index, mine)| {
                let hit = cast_shapes(
                    &Pose::translation(origin.x, origin.y),
                    velocity,
                    &Ball::new(radius),
                    &Pose::translation(mine.position.x, mine.position.y),
                    Vector::ZERO,
                    &Ball::new(MINE_RADIUS),
                    ShapeCastOptions::with_max_time_of_impact(max_time),
                )
                .ok()??;
                Some((hit.time_of_impact, index))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    pub(super) fn shoot_mine(&mut self, index: usize, events: &mut Vec<Event>) {
        let mine = self.mines.remove(index);
        // Shooting disarms the mine without damaging nearby friendly vehicles.
        events.push(Event::Impacted {
            position: mine.position,
            normal: Vector::Y,
        });
    }
}
