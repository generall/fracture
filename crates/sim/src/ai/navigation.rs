//! Navigation uses the remembered map and ordinary vehicle inputs only.
use super::*;

impl Controller {
    pub(super) fn tracked_turn(&self, o: &Observation, error: f32) -> f32 {
        // Own yaw is measured from consecutive own poses. A low-rate kick
        // overcomes contact friction; damping brakes the turn before it
        // overshoots instead of holding a tiny ineffective track difference.
        if error.abs() < 0.025 && self.yaw_rate.abs() < 0.08 {
            return 0.0;
        }
        let request = error * 0.4 - self.yaw_rate * 0.3;
        let minimum = if o.vehicle == VehicleKind::Mortar {
            0.16
        } else {
            0.12
        };
        if error.abs() > 0.05 && self.yaw_rate.abs() < 0.16 && request.signum() == error.signum() {
            error.signum() * request.abs().clamp(minimum, 0.18)
        } else {
            request.clamp(-0.18, 0.18)
        }
    }
    pub(super) fn battle_center(&self) -> Option<Vec2> {
        let home = self
            .map
            .flags
            .iter()
            .find(|(team, _)| *team == self.map.team)?
            .1;
        let enemy = self
            .map
            .flags
            .iter()
            .find(|(team, _)| *team == self.map.team.other())?
            .1;
        Some((home + enemy) * 0.5)
    }
    pub(super) fn navigation_clearance(&self, p: Vec2, vehicle: VehicleKind) -> f32 {
        let radius = 1.36 * vehicle.size().max_element();
        let mut clearance = (self.map.size * 0.5 - p.abs()).min_element() - radius;
        let at = self.map.index(p);
        let w = self.map.width();
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let x = (at % w) as isize + dx;
                let y = (at / w) as isize + dy;
                if x < 0 || y < 0 || x >= w as isize || y >= self.map.height() as isize {
                    continue;
                }
                let i = y as usize * w + x as usize;
                if self.memory[i] >= 2 {
                    clearance = clearance
                        .min(p.distance(self.map.point(i)) - self.map.cell() * 0.71 - radius);
                }
            }
        }
        clearance
    }

    pub(super) fn navigation_corridor(&self, start: Vec2, end: Vec2, vehicle: VehicleKind) -> bool {
        let steps = (start.distance(end) / self.map.cell() * 3.0).ceil() as usize;
        let initial = self.navigation_clearance(start, vehicle).min(0.0);
        (0..=steps).all(|n| {
            let p = start.lerp(end, n as f32 / steps.max(1) as f32);
            let avoids_ally = !self.config.raiding
                || self.traffic_ally.is_none_or(|ally| {
                    p.distance(ally.position) >= start.distance(ally.position).min(3.5) - 0.03
                });
            let avoids_fire = !self.config.raiding
                || (self.prepared_costs[self.map.index(p)]
                    <= self.prepared_costs[self.map.index(start)]
                    && self.threats.iter().all(|(target, _)| {
                        target.mounted
                            || p.distance(target.position)
                                >= start.distance(target.position).min(7.0) - 0.1
                    }));
            let wide = !self.config.wide_routes
                || self.battle_center().is_none_or(|center| {
                    p.distance(center) >= start.distance(center).min(29.0) - 0.1
                });
            wide && avoids_fire
                && avoids_ally
                && self.navigation_clearance(p, vehicle) >= initial - 0.03
        })
    }

    pub(super) fn navigation_controls(&self, o: &Observation, waypoint: Vec2) -> [f32; 2] {
        self.navigation_controls_for(o, waypoint, false)
    }
    pub(super) fn navigation_reverse(&self, o: &Observation, waypoint: Vec2) -> [f32; 2] {
        self.navigation_controls_for(o, waypoint, true)
    }
    pub(super) fn short_drive(&self, o: &Observation, waypoint: Vec2, speed: f32) -> [f32; 2] {
        if o.vehicle == VehicleKind::Scout {
            return self.navigation_controls(o, waypoint);
        }
        let delta = waypoint - o.position;
        if delta.length() < 0.15 {
            return [0.0; 2];
        }
        let heading = delta.to_angle();
        let reverse = angle_error(heading, o.hull_angle).abs() > std::f32::consts::FRAC_PI_2;
        let error = angle_error(
            heading + if reverse { std::f32::consts::PI } else { 0.0 },
            o.hull_angle,
        );
        let forward = if error.abs() < 0.15 {
            speed * if reverse { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        let turn = if self.config.raiding {
            self.tracked_turn(o, error)
        } else if error.abs() > 0.035 {
            error.signum()
                * (error.abs() * 0.16).clamp(
                    if o.vehicle == VehicleKind::Mortar {
                        0.09
                    } else {
                        0.06
                    },
                    0.18,
                )
        } else {
            0.0
        };
        [forward + turn, forward - turn].map(|v| v.clamp(-1.0, 1.0))
    }
    fn navigation_controls_for(
        &self,
        o: &Observation,
        waypoint: Vec2,
        reversing: bool,
    ) -> [f32; 2] {
        let delta = waypoint - o.position;
        if delta.length()
            < if self.config.raiding && o.vehicle != VehicleKind::Scout {
                0.15
            } else {
                0.4
            }
        {
            return [0.0; 2];
        }
        let error = angle_error(delta.to_angle(), o.hull_angle);
        if o.vehicle != VehicleKind::Scout {
            let precision =
                self.config.raiding && o.position.distance(self.telemetry.goal.into()) < 6.0;
            if precision && error.abs() > std::f32::consts::FRAC_PI_2 {
                return self.short_drive(o, waypoint, 0.28);
            }
            // Turn before advancing into a corner. Forward force against a
            // bank can pin a hull and cancel a small differential turn.
            let forward = if error.abs() < if precision { 0.08 } else { 0.35 } {
                self.config.speed.min(if precision { 0.28 } else { 1.0 }) * (1.0 - error.abs())
            } else {
                0.0
            };
            let turn = if self.config.raiding {
                self.tracked_turn(o, error)
            } else if error.abs() > 0.035 {
                let minimum = if o.vehicle == VehicleKind::Mortar {
                    0.09
                } else {
                    0.06
                };
                error.signum() * (error.abs() * 0.16).clamp(minimum, 0.18)
            } else {
                0.0
            };
            return [forward + turn, forward - turn].map(|v| v.clamp(-1.0, 1.0));
        }
        // A scout cannot pivot. Check rolling arcs, including a reverse turn,
        // instead of applying a tank-like turn toward an unreachable waypoint.
        let desired =
            ((2.0 * 1.56 * error.sin() / delta.length().max(1.0)).atan() / 0.5).clamp(-1.0, 1.0);
        let initial = self.navigation_clearance(o.position, o.vehicle).min(0.0);
        let mut best = None;
        let mut throttles = if reversing {
            vec![-0.25, -0.10]
        } else {
            vec![
                if self.config.raiding && delta.length() > 5.0 {
                    0.72
                } else {
                    0.45
                } * self.config.speed,
                0.25,
                -0.25,
            ]
        };
        if !reversing && o.position.distance(self.telemetry.goal.into()) < 8.0 {
            throttles.extend([0.10, 0.05, -0.10]);
        }
        for throttle in throttles {
            for steering in [desired, -1.0, -0.5, 0.0, 0.5, 1.0] {
                let mut valid = true;
                let mut score = f32::INFINITY;
                let scales: &[f32] = if self.config.raiding {
                    &[1.0, 1.225, 1.45]
                } else {
                    &[1.0]
                };
                for scale in scales {
                    let mut position = o.position;
                    let mut heading = o.hull_angle;
                    let mut minimum = f32::INFINITY;
                    for _ in 0..12 {
                        // Include the road multiplier in the scout's swept arc.
                        // At high speed yaw is capped, so its turn radius widens.
                        let speed = throttle * VehicleKind::Scout.speed() * scale;
                        heading += (speed / 1.56 * (steering * 0.5).tan()).clamp(-1.4, 1.4) * 0.15;
                        position += Vec2::from_angle(heading) * speed * 0.15;
                        let clearance = self.navigation_clearance(position, o.vehicle);
                        minimum = minimum.min(clearance);
                        let near_ally = self.config.raiding
                            && self.traffic_ally.is_some_and(|ally| {
                                position.distance(ally.position)
                                    < o.position.distance(ally.position).min(3.5) - 0.03
                            });
                        if clearance < initial - 0.03 || near_ally {
                            valid = false;
                            break;
                        }
                    }
                    if !valid {
                        break;
                    }
                    let cruise = self.config.raiding
                        && !reversing
                        && o.position.distance(self.telemetry.goal.into()) >= 8.0;
                    let progress = if cruise {
                        let motion = position - o.position;
                        motion.dot(delta.normalize())
                            - motion.perp_dot(delta.normalize()).abs() * 0.6
                    } else {
                        delta.length() - position.distance(waypoint)
                    };
                    let desired_heading = if cruise {
                        delta.to_angle()
                    } else {
                        (waypoint - position).to_angle()
                    };
                    let model_score = progress - angle_error(desired_heading, heading).abs() * 1.2
                        + minimum.min(2.0) * 0.3
                        - if throttle < 0.0 { 0.7 } else { 0.0 };
                    score = score.min(model_score);
                }
                if !valid {
                    continue;
                }
                if best.is_none_or(|(old, _)| score > old) {
                    best = Some((
                        score,
                        [(throttle + steering) * 0.5, (throttle - steering) * 0.5],
                    ));
                }
            }
        }
        best.map_or([0.0; 2], |(_, tracks)| tracks)
    }
}
