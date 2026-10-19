//! Aim and fire can never change the movement task or its tracks.
use super::*;

impl Brain {
    pub(super) fn fire_control(
        &mut self,
        o: &Observation,
        task: Task,
        tracks: [f32; 2],
    ) -> PlayerInput {
        let wall = if let Task::Clear(p) = task {
            Some(p)
        } else {
            None
        };
        let solution = self.fire_solution;
        let engaging = solution.filter(|(t, _)| {
            wall.is_none() || (o.health < 85.0 && t.position.distance(o.position) < 15.0)
        });
        let aim = engaging
            .map(|(_, aim)| aim)
            .or(wall)
            .unwrap_or_else(|| match task {
                Task::Travel(_) => self.telemetry.waypoint.into(),
                Task::Face(p) | Task::Enter(p) | Task::Engage(p) => p,
                Task::Yield(_) | Task::Wait if self.passing.is_some() => {
                    self.passing.as_ref().unwrap().last_ally
                }
                _ => self.telemetry.goal.into(),
            });
        let range = aim.distance(o.position);
        let aligned = angle_error((aim - o.position).to_angle(), o.turret_angle).abs() < 0.07;
        let precise_limit = if o.vehicle == VehicleKind::Mortar {
            3.0_f32.to_radians()
        } else {
            (1.5 / range.max(1.0))
                .atan()
                .clamp(4.0_f32.to_radians(), 12.0_f32.to_radians())
        };
        // Steady tracking has a nonzero dispersion floor. Requiring the
        // stationary floor while continuously traversing makes a bot hold
        // fire forever. Estimate this from own motion, just as the reticle
        // communicates it to a human; wall clearing retains its precise limit.
        let tracking_floor = if o.vehicle == VehicleKind::Mortar {
            0.75 + 8.0 * (self.own_speed / o.vehicle.speed()).clamp(0.0, 1.5)
                + 6.0 * self.yaw_rate.abs().min(1.0)
        } else {
            2.0 + 24.0 * (self.own_speed / 6.5).clamp(0.0, 1.5)
                + 8.0 * (self.turret_rate / 60.0_f32.to_radians()).min(1.0)
                + 10.0 * self.yaw_rate.abs().min(1.0)
        };
        let accepted = if self.combined && engaging.is_some() {
            precise_limit.max((tracking_floor + 2.0).to_radians())
        } else {
            precise_limit
        };
        let settled = o.vehicle == VehicleKind::Scout
            || (!self.combined && engaging.is_some() && o.vehicle == VehicleKind::Tank)
            || o.aim_spread < accepted;
        let ally_blocked = o.allies.iter().any(|p| {
            if o.vehicle == VehicleKind::Mortar {
                return p.distance(aim) < 4.0;
            }
            let d = aim - o.position;
            let fraction =
                ((*p - o.position).dot(d) / d.length_squared().max(0.01)).clamp(0.0, 1.0);
            p.distance(o.position + d * fraction) < 1.5
        });
        let acquired = engaging.is_some() || wall.is_some();
        let muzzle_blocked = o.obstacles.iter().any(|b| {
            polygon_distance(
                o.position + Vec2::from_angle(o.turret_angle) * 1.9,
                &b.outline,
            ) < 0.15
        });
        let minimum = if o.vehicle == VehicleKind::Mortar {
            6.5
        } else if wall.is_some() {
            2.1
        } else {
            3.3
        };
        let reason = if !acquired {
            "no target"
        } else if ally_blocked {
            "ally in fire lane"
        } else if engaging.is_some()
            && o.vehicle != VehicleKind::Mortar
            && Controller::blocked_shot(o, aim)
        {
            "predicted fire lane blocked"
        } else if muzzle_blocked {
            "muzzle blocked"
        } else if range <= minimum {
            "inside minimum range"
        } else if o.vehicle == VehicleKind::Mortar && range > 36.0 {
            "predicted target outside weapon range"
        } else if !aligned {
            "aligning"
        } else if !settled {
            "reticle settling"
        } else if o.reload > if self.combined { 0.1 } else { 0.0 } {
            "reloading"
        } else {
            "ready"
        };
        self.telemetry.fire_reason = reason.into();
        let special = o.tick >= self.inventory_ready
            && o.special.is_some_and(|s| {
                if s.charges == 0 || s.cooldown > 0.0 || s.active {
                    return false;
                }
                if s.weapon == SpecialWeapon::Mine {
                    return self.last_mine.is_none_or(|p| p.distance(o.position) > 8.0);
                }
                acquired
                    && aligned
                    && !ally_blocked
                    && !muzzle_blocked
                    && range > 7.0
                    && range < 30.0
                    && (s.weapon != SpecialWeapon::Rocket || visible_impact_range(o, aim) > 7.0)
            });
        if special && o.special.is_some_and(|s| s.weapon == SpecialWeapon::Mine) {
            self.last_mine = Some(o.position);
        }
        PlayerInput {
            tracks,
            aim,
            fire: reason == "ready",
            special,
            ammunition: if o.vehicle == VehicleKind::Mortar {
                Ammunition::HighExplosive
            } else {
                Ammunition::ArmorPiercing
            },
            ..Default::default()
        }
    }
}

fn visible_impact_range(o: &Observation, aim: Vec2) -> f32 {
    let ray = aim - o.position;
    let mut fraction = 1.0_f32;
    for b in &o.obstacles {
        for (a, z) in b
            .outline
            .iter()
            .zip(b.outline.iter().cycle().skip(1))
            .take(b.outline.len())
        {
            let edge = *z - *a;
            let denominator = ray.perp_dot(edge);
            if denominator.abs() < 1e-6 {
                continue;
            }
            let t = (*a - o.position).perp_dot(edge) / denominator;
            let u = (*a - o.position).perp_dot(ray) / denominator;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                fraction = fraction.min(t);
            }
        }
    }
    ray.length() * fraction
}
