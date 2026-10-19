//! Coordination uses public roster slots and delayed visible allied vehicles.
use super::*;

impl Controller {
    pub(super) fn has_traffic_priority(&self, own: VehicleKind, other: VehicleKind) -> bool {
        let rank = |vehicle| match vehicle {
            VehicleKind::Tank => 2,
            VehicleKind::Mortar => 1,
            VehicleKind::Scout => 0,
        };
        rank(own) > rank(other) || (own == other && self.map.team_slot == 0)
    }
    pub(super) fn remember_teammate(&mut self, o: &Observation) {
        if let Some(&ally) = o.allied_vehicles.first() {
            let velocity = self
                .known_ally
                .and_then(|(old, tick, v)| {
                    let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
                    let movement = ally.position - old.position;
                    (dt > 0.0 && dt <= 0.3 && movement.length() <= 12.0 * dt)
                        .then(|| v.lerp(movement / dt, 0.5))
                })
                .unwrap_or(Vec2::ZERO);
            self.known_ally = Some((ally, o.tick, velocity));
        }
        if self.config.raiding {
            if self.life_role == Some("support")
                && o.vehicle != VehicleKind::Scout
                && self.known_ally.is_some_and(|(ally, last, _)| {
                    o.tick.saturating_sub(last) > 1200
                        || self
                            .map
                            .flags
                            .iter()
                            .find(|(t, _)| *t == self.map.team.other())
                            .is_some_and(|(_, flag)| {
                                ally.position.distance(*flag) > o.position.distance(*flag) + 20.0
                            })
                })
            {
                self.life_role = Some("breacher");
                self.support_goal = None;
                self.breach = None;
                self.assault_approach = None;
                self.next_plan = 0;
                self.flank_done = true;
            } else if self.life_role.is_none() && !o.allied_vehicles.is_empty() {
                self.life_role = Some(self.team_role(o));
            }
        }
    }
    pub(super) fn team_role(&self, o: &Observation) -> &'static str {
        if !self.config.team_roles {
            return "independent";
        }
        if self.config.raiding
            && let Some(role) = self.life_role
        {
            return role;
        }
        if self.config.raiding
            && self
                .known_ally
                .is_some_and(|(_, tick, _)| o.tick.saturating_sub(tick) > 1200)
        {
            return "independent";
        }
        let Some(other) = o
            .allied_vehicles
            .first()
            .map(|a| a.vehicle)
            .or_else(|| self.known_ally.map(|(a, _, _)| a.vehicle))
        else {
            return "independent";
        };
        match o.vehicle {
            VehicleKind::Tank if other != VehicleKind::Tank || self.map.team_slot == 0 => {
                "breacher"
            }
            VehicleKind::Mortar
                if other == VehicleKind::Scout
                    || (other == VehicleKind::Mortar && self.map.team_slot == 0) =>
            {
                "breacher"
            }
            VehicleKind::Scout if other == VehicleKind::Scout => {
                if self.map.team_slot == 0 {
                    "flank"
                } else {
                    "guard"
                }
            }
            _ => "support",
        }
    }
    pub(super) fn team_goal(
        &mut self,
        o: &Observation,
        role: &str,
        objective: Vec2,
    ) -> Option<Vec2> {
        if !matches!(role, "support" | "flank" | "guard") {
            self.support_goal = None;
            return None;
        }
        if let Some((goal, until)) = self.support_goal
            && o.tick < until
        {
            return Some(goal);
        }
        let forward = (objective - o.position).normalize_or_zero();
        let goal = if role == "support" {
            if let Some((_, last, _)) = self.known_ally
                && o.tick.saturating_sub(last) > 600
            {
                // Search onward along the public objective route after losing
                // contact. Never park forever at a stale teammate position.
                let search = objective - forward * 20.0 + forward.perp() * 6.0;
                self.support_goal = Some((search, o.tick + 120));
                return Some(search);
            }
            let ally = o
                .allied_vehicles
                .iter()
                .find(|a| a.vehicle != VehicleKind::Scout)
                .copied()
                .or_else(|| {
                    self.known_ally
                        .filter(|(a, _, _)| a.vehicle != VehicleKind::Scout)
                        .map(|(mut a, tick, v)| {
                            a.position += v * (o.tick.saturating_sub(tick) as f32 / 120.0).min(1.5);
                            a
                        })
                })?;
            let axis = (objective - ally.position).normalize_or_zero();
            if self.config.raiding
                && ally.position.distance(objective) > o.position.distance(objective) + 18.0
            {
                // Keep the screen at the front when its leader redeploys. A
                // fresh vehicle at home is not an order to cross the map back.
                let search = objective - forward * 20.0 + forward.perp() * 6.0;
                self.support_goal = Some((search, o.tick + 120));
                return Some(search);
            }
            let behind = if o.vehicle == VehicleKind::Mortar {
                12.0
            } else if o.vehicle == VehicleKind::Scout {
                -10.0
            } else {
                7.0
            };
            [1.0, -1.0]
                .into_iter()
                .map(|side| ally.position - axis * behind + axis.perp() * side * 6.0)
                .filter(|p| p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(2.0)).all())
                .filter(|p| self.memory[self.map.index(*p)] < 2)
                .min_by(|a, b| {
                    a.distance_squared(o.position)
                        .total_cmp(&b.distance_squared(o.position))
                })
                .unwrap_or(ally.position - axis * behind)
        } else if role == "guard" {
            let home = self.map.flags.iter().find(|(t, _)| *t == self.map.team)?.1;
            let direction = (objective - home).normalize_or_zero();
            home + direction * 24.0 + direction.perp() * 6.0
        } else if o.position.distance(objective) < 20.0
            && self.corridor_clear(o.position, objective, o.vehicle)
        {
            objective
        } else {
            let phase = (o.tick / 1200) as u32 + self.map.team_slot as u32 * 4;
            (0..8)
                .map(|i| {
                    objective
                        + Vec2::from_angle((phase + i) as f32 * std::f32::consts::FRAC_PI_4) * 14.0
                })
                .find(|p| {
                    p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(2.0)).all()
                        && self.memory[self.map.index(*p)] < 2
                })
                .unwrap_or(o.position - forward * 3.0)
        };
        self.support_goal = Some((goal, o.tick + 120));
        Some(goal)
    }
    pub(super) fn skirmish_goal(&self, o: &Observation, target: Option<Target>) -> Option<Vec2> {
        if (!self.config.team_roles && !self.config.raiding) || o.vehicle != VehicleKind::Scout {
            return None;
        }
        let target = target.filter(|t| !t.mounted && t.position.distance(o.position) < 24.0)?;
        let radial = (o.position - target.position).normalize_or_zero();
        let preferred = if self.map.team_slot.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        [preferred, -preferred]
            .into_iter()
            .map(|side| target.position + Vec2::from_angle(side * 0.85).rotate(radial) * 9.0)
            .find(|p| self.observed_clear(o, *p) && self.corridor_clear(o.position, *p, o.vehicle))
    }
    pub(super) fn team_traffic(
        &mut self,
        o: &Observation,
        role: &str,
        tracks: [f32; 2],
    ) -> Option<[f32; 2]> {
        if !self.config.team_roles && !self.config.raiding {
            return None;
        }
        if self.config.raiding
            && o.tick < self.yield_reverse_until
            && o.allied_vehicles
                .iter()
                .any(|a| a.position.distance(o.position) < 6.5)
        {
            return Some([-0.25; 2]);
        }
        if let Some((goal, until)) = self.yield_goal {
            let making_room = self.config.raiding
                && o.allied_vehicles.iter().any(|ally| {
                    !self.has_traffic_priority(o.vehicle, ally.vehicle)
                        && ally.position.distance(o.position) < 12.0
                });
            if o.tick < until && (o.position.distance(goal) > 1.0 || making_room) {
                return Some(if o.position.distance(goal) > 1.0 {
                    self.team_drive(o, goal, 0.35)
                } else {
                    [0.0; 2]
                });
            }
            self.yield_goal = None;
        }
        let heading = Vec2::from_angle(o.hull_angle);
        let ally = o
            .allied_vehicles
            .iter()
            .filter(|a| {
                a.position.distance(o.position) < if self.config.raiding { 12.0 } else { 4.0 }
            })
            .min_by(|a, b| {
                a.position
                    .distance_squared(o.position)
                    .total_cmp(&b.position.distance_squared(o.position))
            })?;
        let delta = ally.position - o.position;
        if self.config.raiding && delta.length() > 4.0 && delta.dot(heading.perp()).abs() > 2.5 {
            return None;
        }
        if role == "support"
            && delta.dot(heading) < 0.0
            && delta.length() < if self.config.raiding { 12.0 } else { 4.0 }
            && (!self.config.raiding || !self.has_traffic_priority(o.vehicle, ally.vehicle))
        {
            // Pull ahead and out before attempting a U-turn. Pivoting with a
            // teammate on the rear bumper traps both hulls in contact.
            let preferred =
                if (Vec2::from(self.telemetry.goal) - o.position).dot(heading.perp()) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
            let mut candidates = vec![
                o.position + heading * 6.0 + heading.perp() * preferred * 4.0,
                o.position + heading * 6.0 - heading.perp() * preferred * 4.0,
            ];
            if self.config.raiding {
                candidates.extend([
                    o.position + heading * 10.0 + heading.perp() * preferred * 4.0,
                    o.position + heading * 10.0 - heading.perp() * preferred * 4.0,
                    o.position + heading * 12.0,
                ]);
            }
            for point in candidates {
                if self.observed_clear(o, point)
                    && self.corridor_clear(o.position, point, o.vehicle)
                {
                    self.yield_goal =
                        Some((point, o.tick + if self.config.raiding { 1440 } else { 480 }));
                    return Some(self.team_drive(o, point, 0.65));
                }
            }
        }
        if delta.dot(heading) < 0.0 || tracks.iter().sum::<f32>() <= 0.0 {
            return None;
        }
        let priority = if self.config.raiding {
            self.has_traffic_priority(o.vehicle, ally.vehicle)
        } else {
            role == "breacher" || role == "flank"
        };
        if priority {
            return (delta.length() < if self.config.raiding { 10.0 } else { 2.8 })
                .then_some([0.0; 2]);
        }
        if self.config.raiding
            && self.navigation_corridor(o.position, self.telemetry.waypoint.into(), o.vehicle)
        {
            return None;
        }
        // Support yields right of way and chooses clear space outside the other
        // vehicle's lane. Both bots no longer make the same sidestep together.
        for side in [heading.perp(), -heading.perp(), -delta.normalize_or_zero()] {
            let point = o.position + side * 3.5;
            if point.distance(ally.position) > delta.length() + 0.5
                && self.observed_clear(o, point)
                && self.corridor_clear(o.position, point, o.vehicle)
            {
                self.yield_goal =
                    Some((point, o.tick + if self.config.raiding { 960 } else { 480 }));
                if self.config.raiding
                    && delta.length() < 3.8
                    && self.navigation_corridor(o.position, o.position - heading * 2.5, o.vehicle)
                {
                    self.yield_reverse_until = o.tick + 480;
                    return Some([-0.25; 2]);
                }
                return Some(self.team_drive(o, point, 0.35));
            }
        }
        if self.config.safe_navigation
            && !self.navigation_corridor(o.position, o.position - heading * 2.0, o.vehicle)
        {
            Some([0.0; 2])
        } else {
            Some([-0.25; 2])
        }
    }
    fn team_drive(&self, o: &Observation, goal: Vec2, speed: f32) -> [f32; 2] {
        if self.config.raiding {
            self.short_drive(o, goal, speed)
        } else if self.config.safe_navigation {
            self.navigation_controls(o, goal)
        } else {
            self.steer(o, (goal - o.position).to_angle(), speed)
        }
    }
}
