use super::*;

pub(super) struct Drive {
    pub tracks: [f32; 2],
    pub reason: &'static str,
}

/// A local prediction from measured own motion. No other vehicle's hidden
/// velocity or physics state is available to this model.
struct CarState {
    position: Vec2,
    heading: f32,
    speed: f32,
    yaw: f32,
}
impl CarState {
    fn advance(&mut self, throttle: f32, steering: f32, scale: f32, dt: f32) {
        let target = throttle * VehicleKind::Scout.speed() * scale;
        let braking = throttle == 0.0 || self.speed * (target - self.speed) < 0.0;
        let acceleration = if braking { 8.0 } else { 4.2 * scale };
        self.speed += ((target - self.speed) * (1.0 - (-20.0 * dt).exp()))
            .clamp(-acceleration * dt, acceleration * dt);
        let yaw = (self.speed / 1.56 * (steering * 0.5).tan()).clamp(-1.4, 1.4);
        self.yaw += ((yaw - self.yaw) * (1.0 - (-12.0 * dt).exp())).clamp(-5.0 * dt, 5.0 * dt);
        self.heading += self.yaw * dt;
        self.position += Vec2::from_angle(self.heading) * self.speed * dt;
    }
}

impl Brain {
    /// The only function producing tracks in the task controller.
    pub(super) fn drive(&self, o: &Observation, task: Task, waypoint: Option<Vec2>) -> Drive {
        let tracks = self.tracks(o, task, waypoint);
        let reason = if tracks != [0.0; 2] {
            "control active"
        } else if !task.moving() {
            "task holds position"
        } else if waypoint.is_none() {
            "no complete route"
        } else if waypoint.is_some_and(|p| p.distance(o.position) < 0.4) {
            "waypoint reached"
        } else {
            "no feasible local control"
        };
        Drive { tracks, reason }
    }
    fn tracks(&self, o: &Observation, task: Task, waypoint: Option<Vec2>) -> [f32; 2] {
        match task {
            Task::Engage(p) if o.vehicle == VehicleKind::Mortar && self.combined => {
                // Follow the estimated angular motion as well as position.
                // Pure position damping has a persistent lag behind a moving
                // aim reference and can prevent the fixed gun ever aligning.
                let error = angle_error((p - o.position).to_angle(), o.hull_angle);
                let maximum = if self.nav.local_clearance(o, o.position) >= 0.0 {
                    0.65
                } else {
                    0.18
                };
                let request = error * 1.2 - self.yaw_rate * 0.6 + self.aim_rate * 1.32;
                let turn = if error.abs() > 0.05
                    && self.yaw_rate.abs() < 0.16
                    && request.signum() == error.signum()
                {
                    error.signum() * request.abs().clamp(0.16, maximum)
                } else {
                    request.clamp(-maximum, maximum)
                };
                [turn, -turn]
            }
            Task::Clear(p) | Task::Face(p) | Task::Engage(p)
                if o.vehicle == VehicleKind::Mortar =>
            {
                let turn = self.turn(o, angle_error((p - o.position).to_angle(), o.hull_angle));
                [turn, -turn]
            }
            Task::Clear(_) | Task::Face(_) | Task::Engage(_) | Task::Wait => [0.0; 2],
            _ => {
                let Some(point) = waypoint else {
                    return [0.0; 2];
                };
                if o.vehicle == VehicleKind::Scout {
                    return self.scout_drive(
                        o,
                        point,
                        matches!(task, Task::Travel(_))
                            && o.position.distance(self.telemetry.goal.into()) > 8.0,
                    );
                }
                let delta = point - o.position;
                if delta.length() < 0.18 {
                    return [0.0; 2];
                }
                let reverse = (matches!(task, Task::Yield(_) | Task::Recover(_) | Task::Enter(_))
                    || delta.length() < 4.0
                    || ((angle_error(delta.to_angle(), o.hull_angle).abs()
                        - std::f32::consts::PI)
                        .abs()
                        < 0.04
                        && self.nav.initial_corridor(o, point)))
                    && angle_error(delta.to_angle(), o.hull_angle).abs()
                        > std::f32::consts::FRAC_PI_2;
                let error = angle_error(
                    delta.to_angle() + if reverse { std::f32::consts::PI } else { 0.0 },
                    o.hull_angle,
                );
                let precision = !matches!(task, Task::Travel(_)) || delta.length() < 2.0;
                let throttle = if error.abs() < if precision { 0.15 } else { 0.35 } {
                    self.speed.min(if precision { 0.35 } else { 1.0 })
                        * error.cos().max(0.0)
                        * if reverse { -1.0 } else { 1.0 }
                } else {
                    0.0
                };
                let turn = self.turn(o, error);
                [throttle + turn, throttle - turn].map(|v| v.clamp(-1.0, 1.0))
            }
        }
    }
    fn turn(&self, o: &Observation, error: f32) -> f32 {
        if error.abs() < 0.025 && self.yaw_rate.abs() < 0.08 {
            return 0.0;
        }
        let request = error * 0.4 - self.yaw_rate * 0.3;
        if error.abs() > 0.05 && self.yaw_rate.abs() < 0.16 && request.signum() == error.signum() {
            error.signum()
                * request.abs().clamp(
                    if o.vehicle == VehicleKind::Mortar {
                        0.16
                    } else {
                        0.12
                    },
                    0.18,
                )
        } else {
            request.clamp(-0.18, 0.18)
        }
    }
    fn scout_drive(&self, o: &Observation, point: Vec2, cruise: bool) -> [f32; 2] {
        let delta = point - o.position;
        if delta.length() < 0.4 {
            return [0.0; 2];
        }
        let error = angle_error(delta.to_angle(), o.hull_angle);
        let desired =
            ((2.0 * 1.56 * error.sin() / delta.length().max(1.0)).atan() / 0.5).clamp(-1.0, 1.0);
        let initial = self.nav.local_clearance(o, o.position).min(0.0);
        let risk = self
            .nav
            .public_risk(o.position)
            .max(self.nav.public_risk(point));
        let mut best = None;
        for throttle in [0.72 * self.speed, 0.25, 0.1, 0.0, -0.25, -0.1] {
            let throttle = if cruise {
                throttle
            } else {
                let stopping_speed = (16.0 * (delta.length() - 0.35).max(0.0)).sqrt();
                let limit = stopping_speed / (VehicleKind::Scout.speed() * 1.45);
                throttle.clamp(-limit, limit)
            };
            for steering in [desired, -1.0, -0.5, 0.0, 0.5, 1.0] {
                let mut score = f32::INFINITY;
                let mut valid = true;
                // Own kinematics and known geometry only. Road speed is
                // uncertain, so all three swept arcs must fit.
                for scale in [1.0, 1.225, 1.45] {
                    let mut car = CarState {
                        position: o.position,
                        heading: o.hull_angle,
                        speed: self.own_velocity.dot(Vec2::from_angle(o.hull_angle)),
                        yaw: self.yaw_rate,
                    };
                    let mut clearance = f32::INFINITY;
                    for step in 0..12 {
                        // After the first command, predict braking toward the
                        // arrival region. A constant-speed rollout overshoots
                        // every close goal and alternates forward/reverse even
                        // though the real car could simply roll in and stop.
                        let future_throttle = if cruise || step == 0 {
                            throttle
                        } else {
                            let remaining = point - car.position;
                            let speed = (16.0 * (remaining.length() - 0.35).max(0.0)).sqrt();
                            let limit = speed / (VehicleKind::Scout.speed() * scale);
                            if remaining.dot(Vec2::from_angle(car.heading)) * throttle < 0.0 {
                                0.0
                            } else {
                                throttle.clamp(-limit, limit)
                            }
                        };
                        car.advance(future_throttle, steering, scale, 0.15);
                        let p = car.position;
                        let c = self.nav.local_clearance(o, p);
                        clearance = clearance.min(c);
                        if c < initial - 0.03
                            || self.nav.public_risk(p) > risk
                            || o.allied_vehicles.iter().any(|a| {
                                p.distance(a.position)
                                    < o.position.distance(a.position).min(2.8) - 0.03
                            })
                        {
                            valid = false;
                            break;
                        }
                    }
                    if !valid {
                        break;
                    }
                    let p = car.position;
                    let progress = if cruise {
                        let motion = p - o.position;
                        motion.dot(delta.normalize())
                            - motion.perp_dot(delta.normalize()).abs() * 0.6
                    } else {
                        delta.length() - p.distance(point)
                    };
                    let desired_heading = if cruise {
                        delta.to_angle()
                    } else {
                        (point - p).to_angle()
                    } + if throttle < 0.0 {
                        std::f32::consts::PI
                    } else {
                        0.0
                    };
                    let heading_cost = if !cruise && p.distance(point) < 0.8 {
                        0.0
                    } else {
                        angle_error(desired_heading, car.heading).abs() * 1.2
                    };
                    let s =
                        progress - heading_cost - if cruise { 0.0 } else { car.speed.abs() * 0.15 }
                            + clearance.min(2.0) * 0.3
                            - if throttle < 0.0 { 0.7 } else { 0.0 };
                    score = score.min(s);
                }
                if valid && best.is_none_or(|(old, _)| score > old) {
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
