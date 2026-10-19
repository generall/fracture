use super::*;

impl Controller {
    pub(super) fn movement_recovery(
        &mut self,
        o: &Observation,
        waypoint: Vec2,
        mut tracks: [f32; 2],
        mut mode: &'static str,
    ) -> ([f32; 2], &'static str) {
        self.avoid.retain(|(_, until, _)| *until > o.tick);
        let hold = matches!(
            mode,
            "combat"
                | "returning fire"
                | "engaging vehicle"
                | "covering teammate"
                | "yielding to teammate"
                | "clearing"
                | "mortar aiming"
                | "waiting for clear impact"
        );
        if hold {
            self.history.clear();
            self.progress_at = o.position;
            self.progress_heading = o.hull_angle;
            self.progress_tick = o.tick;
            self.reverse_until = 0;
            return (tracks, mode);
        }
        if self.config.recovery_detour {
            self.history.push_back((o.tick, o.position));
            while self
                .history
                .front()
                .is_some_and(|(tick, _)| o.tick.saturating_sub(*tick) > 1200)
            {
                self.history.pop_front();
            }
            if self.history.front().is_some_and(|(tick, p)| {
                o.tick.saturating_sub(*tick) >= 1188 && o.position.distance(*p) < 3.0
            }) {
                self.avoid.push((
                    waypoint,
                    o.tick + 2400,
                    if self.vehicle == VehicleKind::Scout && self.config.bounded_clearing {
                        2
                    } else {
                        0
                    },
                ));
                self.scout_patrol += 1;
                self.next_plan = 0;
                self.history.clear();
                self.reverse_until = o.tick + 120;
                self.telemetry.recoveries += 1;
            }
        }
        if o.position.distance(self.progress_at) > 0.5
            || angle_error(o.hull_angle, self.progress_heading).abs() > 0.35
        {
            self.progress_at = o.position;
            self.progress_heading = o.hull_angle;
            self.progress_tick = o.tick;
        }
        if o.tick.saturating_sub(self.progress_tick) > (self.config.recover_seconds * 120.0) as u64
        {
            self.reverse_until = o.tick + 144;
            self.progress_tick = o.tick;
            self.next_plan = 0;
            self.telemetry.recoveries += 1;
        }
        if o.tick < self.reverse_until {
            if self.config.safe_navigation {
                let rear = o.position - Vec2::from_angle(o.hull_angle) * 2.5;
                if self.navigation_corridor(o.position, rear, o.vehicle) {
                    if o.vehicle == VehicleKind::Scout {
                        let goal = if waypoint.distance(o.position) > 0.4 {
                            waypoint
                        } else {
                            rear
                        };
                        return (self.navigation_reverse(o, goal), "recovering");
                    }
                    return ([-0.3; 2], "recovering");
                }
                return (self.navigation_controls(o, waypoint), "recovering");
            }
            tracks = [-0.35, -0.22];
            mode = "recovering";
            let rear = o.position - Vec2::from_angle(o.hull_angle) * 1.5;
            let blocked = if self.config.siege_tactics {
                self.memory[self.map.index(rear)] >= 3
                    || o.obstacles
                        .iter()
                        .any(|b| !b.breakable && polygon_distance(rear, &b.outline) < 1.0)
            } else {
                !self.observed_clear(o, rear)
            };
            if self.config.adaptive_recovery && blocked {
                tracks = if self.config.bounded_clearing && o.vehicle == VehicleKind::Scout {
                    if self.observed_clear(o, o.position + Vec2::from_angle(o.hull_angle) * 1.5) {
                        self.steer(o, o.hull_angle + 0.9, 0.35)
                    } else {
                        [-0.12, -0.30]
                    }
                } else if self.config.siege_tactics && o.vehicle == VehicleKind::Mortar {
                    [0.23, -0.23]
                } else {
                    [0.12, -0.12]
                };
            }
        }
        (tracks, mode)
    }
}
