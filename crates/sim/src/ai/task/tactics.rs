//! Mission and combat choices use the public map and delayed sightings only.
use super::*;

impl Brain {
    pub(super) fn observe_teammate(&mut self, o: &Observation) {
        let Some(ally) = o.allied_vehicles.first().copied() else {
            return;
        };
        let mut motion = Vec2::ZERO;
        if let Some((old, tick, velocity)) = self.teammate {
            let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
            let delta = ally.position - old.position;
            if old.vehicle == ally.vehicle && dt > 0.0 && dt <= 0.5 && delta.length() < 15.0 * dt {
                motion = if delta.length() > 0.02 {
                    velocity.lerp(delta / dt, 0.5)
                } else {
                    velocity
                };
            }
        }
        self.teammate = Some((ally, o.tick, motion));
    }
    pub(super) fn should_flank(&self, o: &Observation) -> bool {
        if o.vehicle == VehicleKind::Scout {
            return false;
        }
        match o.allied_vehicles.first().map(|a| a.vehicle) {
            Some(v) if v == o.vehicle => self.nav.map.team_slot == 0,
            Some(VehicleKind::Tank) => false,
            Some(_) => true,
            None => o.vehicle == VehicleKind::Tank || self.nav.map.team_slot == 0,
        }
    }
    pub(super) fn flank_station(
        &mut self,
        o: &Observation,
        station: Vec2,
    ) -> Option<(Vec2, Vec<Vec2>)> {
        let home = self
            .nav
            .map
            .flags
            .iter()
            .find(|(t, _)| *t == self.nav.map.team)?
            .1;
        let enemy = self
            .nav
            .map
            .flags
            .iter()
            .find(|(t, _)| *t == self.nav.map.team.other())?
            .1;
        let axis = (enemy - home).normalize_or_zero();
        let desired = (home + enemy) * 0.5 + axis.perp() * self.nav.map.size.min_element() * 0.34;
        let mut choices: Vec<_> = (-2..=2)
            .flat_map(|y| (-2..=2).map(move |x| desired + Vec2::new(x as f32, y as f32) * 4.0))
            .filter(|p| self.nav.clearance(*p, o.vehicle) > 0.5 && self.nav.public_safe(*p))
            .collect();
        choices.sort_by(|a, b| {
            a.distance_squared(desired)
                .total_cmp(&b.distance_squared(desired))
        });
        for point in choices.into_iter().take(3) {
            self.telemetry.replans += 2;
            if let Plan::Complete(mut first) = self.nav.plan_observed(o, point) {
                // Do not complete an arbitrary side coordinate after its
                // route has already got closer to the base and turned away.
                let best = first
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| self.nav.public_safe(**p))
                    .min_by(|(_, a), (_, b)| {
                        a.distance_squared(station)
                            .total_cmp(&b.distance_squared(station))
                    })
                    .map(|(i, _)| i)?;
                let checkpoint = first[best];
                first.truncate(best + 1);
                if matches!(
                    self.nav.plan(checkpoint, station, o.vehicle),
                    Plan::Complete(_)
                ) {
                    return Some((checkpoint, first));
                }
            }
        }
        None
    }
    pub(in crate::ai) fn aim_path(&mut self, input: PlayerInput) -> Vec<Vec2> {
        if !self.combined || self.kind.is_none() {
            self.aim_anchor = None;
            return vec![];
        }
        let start = self.aim_anchor.unwrap_or(input.aim);
        self.aim_anchor = Some(input.aim);
        (1..=12)
            .map(|step| start.lerp(input.aim, step as f32 / 12.0))
            .collect()
    }
    pub(super) fn prepare_fire(&mut self, o: &Observation) {
        self.aim_rate = 0.0;
        let target = self.select_fire_target(o);
        self.fire_solution = target.map(|target| {
            let mut velocity = Vec2::ZERO;
            if self.combined
                && !target.mounted
                && let Some((old, tick, prior)) = self.tracked
            {
                let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
                let delta = target.position - old.position;
                if !old.mounted
                    && old.vehicle == target.vehicle
                    && dt > 0.0
                    && dt <= 0.3
                    && delta.length() <= 12.0 * dt
                {
                    velocity = prior.lerp(delta / dt, 0.5);
                }
            }
            self.tracked = Some((target, o.tick, velocity));
            let range = target.position.distance(o.position);
            let flight = if o.vehicle == VehicleKind::Mortar {
                0.8 + range / 24.0
            } else {
                range / 260.0
            };
            let delta = target.position - o.position;
            self.aim_rate = (delta.perp_dot(velocity - self.own_velocity)
                / delta.length_squared().max(1.0))
            .clamp(-1.0, 1.0);
            (
                target,
                target.position + velocity * (REACTION_TICKS as f32 / 120.0 + flight).min(2.0),
            )
        });
        if target.is_none() {
            self.tracked = None;
        }
    }
    pub(super) fn select_fire_target(&self, o: &Observation) -> Option<Target> {
        o.targets
            .iter()
            .filter(|t| {
                let range = t.position.distance(o.position);
                range
                    > if o.vehicle == VehicleKind::Mortar {
                        6.5
                    } else {
                        3.3
                    }
                    && range
                        < if o.vehicle == VehicleKind::Mortar {
                            36.0
                        } else {
                            32.0
                        }
                    && (o.vehicle == VehicleKind::Mortar
                        || !Controller::blocked_shot(o, t.position))
            })
            .min_by(|a, b| {
                let priority = |t: &&Target| {
                    self.combined
                        && o.vehicle == VehicleKind::Scout
                        && t.vehicle == Some(VehicleKind::Mortar)
                };
                priority(b).cmp(&priority(a)).then_with(|| {
                    a.position
                        .distance_squared(o.position)
                        .total_cmp(&b.position.distance_squared(o.position))
                })
            })
            .copied()
    }
    pub(super) fn combat_task(
        &mut self,
        o: &Observation,
        wanted: Task,
        patrol: bool,
    ) -> Option<Task> {
        if self.perimeter
            && self
                .mission
                .as_ref()
                .is_some_and(|m| m.via.is_some() && o.position.distance(m.objective) > 32.0)
            && o.health >= 45.0
            && self.fire_solution.is_none_or(|(t, _)| {
                !t.mounted && self.tracked.is_some_and(|(_, _, v)| v.length() > 1.0)
            })
        {
            self.combat_since = None;
            return None;
        }
        if !self.combined
            || patrol
            || !self.assault
            || matches!(wanted, Task::Clear(_) | Task::Enter(_) | Task::Recover(_))
        {
            self.combat_since = None;
            return None;
        }
        let Some((target, aim)) = self.fire_solution else {
            self.combat_since = None;
            return None;
        };
        if o.tick < self.combat_ready {
            return None;
        }
        let range = target.position.distance(o.position);
        if range
            > if o.vehicle == VehicleKind::Mortar {
                35.0
            } else {
                24.0
            }
        {
            self.combat_since = None;
            return None;
        }
        let since = *self.combat_since.get_or_insert(o.tick);
        // An encounter cannot permanently replace the strategic mission.
        // A failed exchange gets a travel window before another firing hold.
        if o.tick.saturating_sub(since) > 1440 {
            self.combat_ready = o.tick + 720;
            self.combat_since = None;
            return None;
        }
        Some(Task::Engage(aim))
    }
    pub(super) fn scout_station(
        &mut self,
        o: &Observation,
        path: &[Vec2],
    ) -> Option<(Vec2, Vec<Vec2>)> {
        let mut previous = o.position;
        let length: f32 = path
            .iter()
            .map(|p| {
                let d = p.distance(previous);
                previous = *p;
                d
            })
            .sum();
        let mut traversed = 0.0;
        previous = o.position;
        let (point, axis) = path.iter().find_map(|p| {
            traversed += p.distance(previous);
            let axis = (*p - previous).normalize_or_zero();
            previous = *p;
            (traversed >= length * 0.4).then_some((*p, axis))
        })?;
        let preferred = if self.nav.map.team_slot == 0 {
            1.0
        } else {
            -1.0
        };
        for distance in [12.0, 9.0, 6.0] {
            for side in [preferred, -preferred] {
                let station = point + axis.perp() * side * distance;
                if self.nav.clearance(station, o.vehicle) < 0.0 || !self.nav.public_safe(station) {
                    continue;
                }
                self.telemetry.replans += 1;
                if let Plan::Complete(route) = self.nav.plan_observed(o, station) {
                    return Some((station, route));
                }
            }
        }
        None
    }
}
