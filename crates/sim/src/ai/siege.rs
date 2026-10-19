use super::*;

impl Controller {
    pub(super) fn assault_goal(
        &mut self,
        o: &Observation,
        goal: Vec2,
        objective: Vec2,
        role: &str,
    ) -> Vec2 {
        if self.patrol_goal.is_some()
            || !self.config.siege_tactics
            || !self.config.advance_objective
            || o.vehicle == VehicleKind::Scout
            || !matches!(role, "breacher" | "independent")
            || (!self.config.raiding && o.position.distance(objective) > 24.0)
        {
            return goal;
        }
        if let Some(breach) = self.breach {
            return if self.config.raiding {
                breach.objective
            } else {
                objective
            };
        }
        if self.corridor_clear(o.position, objective, o.vehicle) {
            return goal;
        }
        if self.assault_approach.is_none() {
            // Approach a face squarely. A diagonal shortcut through an
            // enclosure's corner pillars can never become a usable breach.
            let offsets: &[f32] = if !self.config.raiding {
                &[14.0]
            } else if self.config.raiding
                && o.special
                    .is_some_and(|s| s.weapon == SpecialWeapon::Rocket && s.charges > 0)
            {
                &[17.0, 15.0, 14.0]
            } else {
                &[14.0, 13.0, 12.0]
            };
            let valid = |p: Vec2| {
                p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(1.5)).all()
                    && self.memory[self.map.index(p)] < 2
                    && (!self.config.raiding
                        || (self.navigation_clearance(p, o.vehicle) > 0.0
                            && self.prepared_costs[self.map.index(p)] == 0))
                    && {
                        (0..=42).all(|n| {
                            let point = p.lerp(objective, n as f32 / 42.0);
                            self.memory[self.map.index(point)] < 3
                                && !self.avoid.iter().any(|(a, until, r)| {
                                    *r > 0 && *until > o.tick && a.distance(point) < 2.5
                                })
                        })
                    }
            };
            let mut candidates: Vec<_> = [Vec2::X, Vec2::Y, -Vec2::X, -Vec2::Y]
                .into_iter()
                .filter_map(|axis| {
                    offsets
                        .iter()
                        .map(|d| objective + axis * *d)
                        .find(|p| valid(*p))
                })
                .collect();
            candidates.sort_by(|a, b| {
                a.distance_squared(o.position)
                    .total_cmp(&b.distance_squared(o.position))
            });
            self.assault_approach = if self.config.raiding {
                // Compare reachable routes, including the public gun sectors,
                // rather than selecting a face by straight-line distance.
                // Bound this to two searches per life/approach change.
                candidates
                    .iter()
                    .take(2)
                    .filter_map(|&p| {
                        let (route, cost) = self.plan_with_cost(o.position, p);
                        (route.last().copied().unwrap_or(o.position).distance(p)
                            < self.map.cell() * 2.0)
                            .then_some((p, cost))
                    })
                    .min_by_key(|(_, cost)| *cost)
                    .map(|(p, _)| p)
                    .or_else(|| candidates.first().copied())
            } else {
                candidates.first().copied()
            };
        }
        if let Some(approach) = self.assault_approach {
            if o.position.distance(approach) < if self.config.raiding { 0.4 } else { 2.5 } {
                self.breach = Some(Breach {
                    objective,
                    exit: objective,
                    since: o.tick,
                    shots: 0,
                    last_reload: o.reload,
                    drive_until: 0,
                });
                self.route.clear();
                objective
            } else {
                approach
            }
        } else {
            goal
        }
    }
}
