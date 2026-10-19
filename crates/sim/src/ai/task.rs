//! A task executive, not a chain of movement overrides. The selected task is
//! the only owner of tracks; fire control receives it read-only.
use super::*;
mod driver;
mod fire;
mod route;
mod tactics;
#[cfg(test)]
mod tests;
use route::{Navigation, Plan};

#[derive(Clone, Copy, Debug)]
enum Task {
    Travel(Vec2),
    Enter(Vec2),
    Clear(Vec2),
    Face(Vec2),
    Engage(Vec2),
    Yield(Vec2),
    Recover(Vec2),
    Wait,
}
impl Task {
    fn label(self) -> &'static str {
        match self {
            Self::Travel(_) => "travel",
            Self::Enter(_) => "enter/capture",
            Self::Clear(_) => "clear wall",
            Self::Face(_) => "observe/aim",
            Self::Engage(_) => "engage",
            Self::Yield(_) => "yield",
            Self::Recover(_) => "recover",
            Self::Wait => "wait",
        }
    }
    fn moving(self) -> bool {
        matches!(
            self,
            Self::Travel(_) | Self::Enter(_) | Self::Yield(_) | Self::Recover(_)
        )
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Flank,
    Approach,
    Breach,
    Enter,
    Screen,
}
struct Mission {
    objective: Vec2,
    station: Vec2,
    phase: Phase,
    via: Option<Vec2>,
}
struct Passing {
    bay: Vec2,
    axis: Vec2,
    since: u64,
    last_ally: Vec2,
    last_seen: u64,
}
struct Clearing {
    since: u64,
    started: u64,
    occupied: Vec<bool>,
    failed_shots: u32,
    reload: f32,
}
struct Recovery {
    point: Vec2,
    until: u64,
    reason: &'static str,
}
struct CachedRoute {
    goal: Vec2,
    points: Vec<Vec2>,
    revision: u64,
    retry_at: u64,
}

pub(super) struct Brain {
    nav: Navigation,
    speed: f32,
    assault: bool,
    combined: bool,
    perimeter: bool,
    fire_solution: Option<(Target, Vec2)>,
    tracked: Option<(Target, u64, Vec2)>,
    combat_since: Option<u64>,
    combat_ready: u64,
    aim_anchor: Option<Vec2>,
    mission: Option<Mission>,
    route: Option<CachedRoute>,
    passing: Option<Passing>,
    teammate: Option<(AlliedVehicle, u64, Vec2)>,
    clearing: Option<Clearing>,
    recovery: Option<Recovery>,
    previous: Option<(u64, Vec2, f32)>,
    yaw_rate: f32,
    own_speed: f32,
    own_velocity: Vec2,
    aim_rate: f32,
    turret_rate: f32,
    previous_turret: Option<(u64, f32)>,
    progress: Option<(u64, Vec2, f32)>,
    random: u64,
    deployment: Option<VehicleKind>,
    kind: Option<VehicleKind>,
    failed_faces: Vec<Vec2>,
    next_mission: u64,
    last_mine: Option<Vec2>,
    inventory: Option<SpecialWeapon>,
    inventory_ready: u64,
    pub telemetry: Telemetry,
}
impl Brain {
    pub(super) fn use_perimeter(&mut self, enabled: bool) {
        self.perimeter = enabled;
    }
    pub fn new(map: Map, speed: f32, assault: bool, combined: bool) -> Self {
        Self {
            random: map.seed ^ 0x9e3779b97f4a7c15 ^ if map.team == Team::Blue { 42 } else { 137 },
            nav: Navigation::new(map),
            speed,
            assault,
            combined,
            perimeter: false,
            fire_solution: None,
            tracked: None,
            combat_since: None,
            combat_ready: 0,
            aim_anchor: None,
            mission: None,
            route: None,
            passing: None,
            teammate: None,
            clearing: None,
            recovery: None,
            previous: None,
            yaw_rate: 0.0,
            own_speed: 0.0,
            own_velocity: Vec2::ZERO,
            aim_rate: 0.0,
            turret_rate: 0.0,
            previous_turret: None,
            progress: None,
            deployment: None,
            kind: None,
            failed_faces: vec![],
            next_mission: 0,
            last_mine: None,
            inventory: None,
            inventory_ready: 0,
            telemetry: Telemetry::default(),
        }
    }
    pub fn decide(&mut self, o: &Observation, patrol: Option<Vec2>) -> PlayerInput {
        if !o.alive {
            self.mission = None;
            self.route = None;
            self.passing = None;
            self.teammate = None;
            self.clearing = None;
            self.recovery = None;
            self.progress = None;
            self.previous = None;
            self.previous_turret = None;
            self.fire_solution = None;
            self.tracked = None;
            self.combat_since = None;
            self.combat_ready = 0;
            self.aim_anchor = None;
            self.kind = None;
            self.failed_faces.clear();
            self.next_mission = 0;
            self.telemetry.mode = "respawning".into();
            self.telemetry.moving = false;
            if self.deployment.is_none() {
                self.random = self.random.wrapping_add(0x9e3779b97f4a7c15);
                let mut n = self.random;
                n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
                self.deployment = Some(VehicleKind::ALL[((n ^ (n >> 31)) % 3) as usize]);
            }
            return PlayerInput {
                deploy: self.deployment,
                ..Default::default()
            };
        }
        self.deployment = None;
        self.own_velocity = self
            .previous
            .and_then(|(tick, p, _)| {
                let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
                (dt > 0.0 && dt <= 0.5).then(|| (o.position - p) / dt)
            })
            .unwrap_or(Vec2::ZERO);
        self.own_speed = self.own_velocity.length();
        self.turret_rate = self
            .previous_turret
            .and_then(|(tick, angle)| {
                let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
                (dt > 0.0 && dt <= 0.5).then(|| angle_error(o.turret_angle, angle).abs() / dt)
            })
            .unwrap_or(0.0);
        self.previous_turret = Some((o.tick, o.turret_angle));
        self.yaw_rate = self
            .previous
            .and_then(|(tick, _, heading)| {
                let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
                (dt > 0.0 && dt <= 0.5)
                    .then(|| (angle_error(o.hull_angle, heading) / dt).clamp(-3.0, 3.0))
            })
            .unwrap_or(0.0);
        self.previous = Some((o.tick, o.position, o.hull_angle));
        if self.kind != Some(o.vehicle) {
            self.kind = Some(o.vehicle);
            self.mission = None;
            self.route = None;
        }
        if self.inventory != o.special.map(|s| s.weapon) {
            self.inventory = o.special.map(|s| s.weapon);
            self.inventory_ready = o.tick + REACTION_TICKS;
        }
        self.nav.observe(o);
        self.observe_teammate(o);
        self.prepare_fire(o);
        self.telemetry.role = match o.vehicle {
            VehicleKind::Scout if self.combined => "interceptor",
            VehicleKind::Scout => "screen",
            VehicleKind::Tank => "assault",
            VehicleKind::Mortar => "siege",
        }
        .into();
        self.telemetry.known_cells = self.nav.cells.iter().filter(|&&v| v > 0).count();
        self.telemetry.route_status = "not required".into();
        self.telemetry.decision = "follow mission route".into();

        let wanted = self.mission_task(o, patrol);
        let intended = match wanted {
            Task::Travel(goal) => self.travel_waypoint(o, goal),
            Task::Enter(p) => Some(p),
            _ => None,
        };
        if self.mission.as_ref().is_some_and(|m| m.via.is_some()) {
            self.telemetry.role = "flanker".into();
        }
        // One arbitration point, then one call that renders tracks. No weapon
        // or mission code below this point can replace the movement command.
        let task = if let Some(task) = self.traffic_task(o, intended) {
            self.progress = None;
            self.telemetry.decision = "persistent right of way".into();
            task
        } else if let Some(task) = self.recovery_task(o, wanted) {
            self.telemetry.decision = self
                .recovery
                .as_ref()
                .map_or("movement progress deadline", |r| r.reason)
                .into();
            task
        } else if let Some(task) = self.combat_task(o, wanted, patrol.is_some()) {
            self.progress = None;
            self.telemetry.decision = "engage visible threat before continuing mission".into();
            task
        } else {
            wanted
        };
        self.telemetry.mode = task.label().into();
        let waypoint = match task {
            Task::Travel(_) => intended,
            Task::Enter(p) | Task::Yield(p) | Task::Recover(p) => Some(p),
            _ => None,
        };
        self.telemetry.waypoint = waypoint.unwrap_or(o.position).to_array();
        let drive = self.drive(o, task, waypoint);
        let tracks = drive.tracks;
        self.telemetry.drive_reason = drive.reason.into();
        self.telemetry.route_cells = self.route.as_ref().map_or(0, |r| r.points.len());
        self.telemetry.moving = tracks != [0.0; 2];
        self.fire_control(o, task, tracks)
    }
    fn mission_task(&mut self, o: &Observation, patrol: Option<Vec2>) -> Task {
        let objective = patrol
            .or_else(|| {
                self.nav
                    .map
                    .flags
                    .iter()
                    .find(|(t, _)| *t != self.nav.map.team)
                    .map(|(_, p)| *p)
            })
            .unwrap_or(o.position);
        if patrol.is_some() || !self.assault {
            self.telemetry.goal = objective.to_array();
            return if o.position.distance(objective) < 0.6 {
                Task::Wait
            } else {
                Task::Travel(objective)
            };
        }
        if self.mission.is_none() && o.tick >= self.next_mission {
            if self.perimeter && o.regions.is_empty() {
                self.telemetry.goal = objective.to_array();
                self.telemetry.decision =
                    "wait for delayed teammate sighting before assigning route".into();
                return Task::Face(objective);
            }
            self.choose_approach(o, objective);
        }
        let Some(m) = &mut self.mission else {
            self.telemetry.goal = objective.to_array();
            self.telemetry.route_status = "no reachable approach".into();
            self.telemetry.decision = "wait for reachable approach".into();
            return Task::Face(objective);
        };
        if m.objective.distance(objective) > 0.5 {
            self.mission = None;
            self.route = None;
            return Task::Wait;
        }
        if m.phase == Phase::Flank {
            let via = m.via.unwrap();
            self.telemetry.goal = via.to_array();
            if o.position.distance(via) >= 3.0 {
                return Task::Travel(via);
            }
            m.phase = Phase::Approach;
            self.route = None;
        }
        if m.phase == Phase::Approach
            && o.position.distance(m.station)
                < if self.combined && o.vehicle == VehicleKind::Scout {
                    2.0
                } else {
                    0.8
                }
        {
            m.phase = if o.vehicle == VehicleKind::Scout {
                Phase::Screen
            } else {
                Phase::Breach
            };
            self.route = None;
        }
        let (station, phase) = (m.station, m.phase);
        self.telemetry.goal =
            if phase == Phase::Approach || (self.combined && phase == Phase::Screen) {
                station
            } else {
                objective
            }
            .to_array();
        match phase {
            Phase::Flank => unreachable!("flank handled before base approach"),
            Phase::Approach => Task::Travel(station),
            Phase::Breach | Phase::Enter => {
                if self.exact_corridor(o, objective) {
                    self.telemetry.decision = "hull corridor open".into();
                    self.mission.as_mut().unwrap().phase = Phase::Enter;
                    self.clearing = None;
                    Task::Enter(objective)
                } else if let Some(point) = self.wall_on_axis(o, objective) {
                    // A wall task owns a safe stationary firing position. A
                    // later muzzle rule cannot send it driving into the wall.
                    if o.position.distance(point)
                        < if o.vehicle == VehicleKind::Mortar {
                            9.0
                        } else {
                            2.6
                        }
                    {
                        // Once inside a breach, coarse travel inflation can
                        // seal the entrance behind us. Back along the observed
                        // firing line for a few metres; do not ask the global
                        // planner to route all the way outside again.
                        let back = o.position + (o.position - point).normalize_or_zero() * 3.0;
                        if self.nav.initial_corridor(o, back) {
                            self.telemetry.decision = "back out to firing standoff".into();
                            Task::Recover(back)
                        } else {
                            self.telemetry.decision = "no clear standoff retreat".into();
                            Task::Clear(point)
                        }
                    } else if self.clearing_failed(o, station, objective) {
                        self.telemetry.decision = "breach stalled; change approach".into();
                        self.failed_faces.push(station);
                        self.mission = None;
                        self.route = None;
                        self.clearing = None;
                        self.next_mission = o.tick + 120;
                        Task::Wait
                    } else {
                        self.telemetry.decision = "visible wall blocks hull corridor".into();
                        Task::Clear(point)
                    }
                } else {
                    self.telemetry.decision = "observe blocked corridor".into();
                    // Need to observe the corridor or a solid post blocks it.
                    // Do not interpret missing visible walls as a clear path.
                    Task::Face(objective)
                }
            }
            Phase::Screen => {
                self.telemetry.decision = "scout screens; main gun cannot breach walls".into();
                if self.exact_corridor(o, objective) {
                    Task::Enter(objective)
                } else {
                    Task::Face(objective)
                }
            }
        }
    }
    fn choose_approach(&mut self, o: &Observation, objective: Vec2) {
        let mut best: Option<(f32, Vec2, Vec<Vec2>)> = None;
        for (side, axis) in [Vec2::X, Vec2::Y, -Vec2::X, -Vec2::Y]
            .into_iter()
            .enumerate()
        {
            let candidate = [14.0, 13.0, 12.0]
                .into_iter()
                .map(|d| objective + axis * d)
                .find(|p| {
                    self.nav.clearance(*p, o.vehicle) > 0.0
                        && self.nav.public_safe(*p)
                        && !self.failed_faces.iter().any(|old| old.distance(*p) < 3.0)
                        && (0..=42).all(|n| {
                            self.nav.cells[self.nav.map.index(p.lerp(objective, n as f32 / 42.0))]
                                < 3
                        })
                });
            let Some(goal) = candidate else {
                continue;
            };
            self.telemetry.replans += 1;
            if let Plan::Complete(path) = self.nav.plan_observed(o, goal) {
                let mut last = o.position;
                let length: f32 = path
                    .iter()
                    .map(|p| {
                        let d = last.distance(*p);
                        last = *p;
                        d
                    })
                    .sum();
                // Equal routes distribute slots between faces without chasing
                // another vehicle's continuously changing position.
                let score = length
                    + if side % 2 == self.nav.map.team_slot % 2 {
                        0.0
                    } else {
                        4.0
                    };
                if best.as_ref().is_none_or(|(old, _, _)| score < *old) {
                    best = Some((score, goal, path));
                }
            }
        }
        if let Some((_, station, points)) = best {
            let (station, points) = if self.combined && o.vehicle == VehicleKind::Scout {
                self.scout_station(o, &points).unwrap_or((station, points))
            } else {
                (station, points)
            };
            let flank = if self.perimeter && self.should_flank(o) {
                self.flank_station(o, station)
            } else {
                None
            };
            let (via, route_goal, points) = flank.map_or((None, station, points), |(via, path)| {
                (Some(via), via, path)
            });
            self.mission = Some(Mission {
                objective,
                station,
                phase: if via.is_some() {
                    Phase::Flank
                } else {
                    Phase::Approach
                },
                via,
            });
            self.route = Some(CachedRoute {
                goal: route_goal,
                points,
                revision: self.nav.revision,
                retry_at: 0,
            });
        } else {
            self.telemetry.route_status = "no reachable approach".into();
            self.next_mission = o.tick + 960;
            if !self.failed_faces.is_empty() {
                self.failed_faces.clear();
            }
        }
    }
    fn travel_waypoint(&mut self, o: &Observation, goal: Vec2) -> Option<Vec2> {
        if self
            .route
            .as_ref()
            .is_none_or(|r| r.goal.distance(goal) > 0.1)
        {
            self.route = None;
        }
        if let Some(r) = &mut self.route {
            while r.points.first().is_some_and(|p| {
                p.distance(o.position) < if r.points.len() == 1 { 0.35 } else { 0.8 }
            }) {
                r.points.remove(0);
            }
            if !r.points.is_empty() && r.revision != self.nav.revision {
                if !self.nav.initial_corridor(o, r.points[0]) {
                    self.route = None;
                } else {
                    r.revision = self.nav.revision;
                }
            }
        }
        let retry = self.route.as_ref().is_none_or(|r| {
            r.points.is_empty()
                && o.position.distance(goal) > 0.8
                && (o.tick >= r.retry_at || r.revision != self.nav.revision)
        });
        if retry {
            self.telemetry.replans += 1;
            let (points, retry_at) = match self.nav.plan_observed(o, goal) {
                Plan::Complete(path) => (path, 0),
                Plan::Unreachable { nearest } => {
                    self.telemetry.decision =
                        format!("unreachable; nearest {:.1},{:.1}", nearest.x, nearest.y);
                    (vec![], o.tick + 960)
                }
            };
            self.route = Some(CachedRoute {
                goal,
                points,
                revision: self.nav.revision,
                retry_at,
            });
        }
        let r = self.route.as_mut()?;
        if r.points.is_empty() {
            self.telemetry.route_status = if o.position.distance(goal) < 0.8 {
                "arrived"
            } else {
                "unreachable"
            }
            .into();
            return None;
        }
        self.telemetry.route_status = "complete".into();
        let selected = r
            .points
            .iter()
            .take(9)
            .enumerate()
            .take_while(|(_, p)| {
                p.distance(o.position) <= 6.0 && self.nav.route_corridor(o.position, **p, o.vehicle)
            })
            .last()
            .map_or(0, |(i, _)| i);
        // A proven clear shortcut consumes the route prefix. Otherwise an
        // overshot cell behind the hull can pull the driver backwards later.
        r.points.drain(..selected);
        r.points.first().copied()
    }
    fn exact_corridor(&self, o: &Observation, goal: Vec2) -> bool {
        let steps = (o.position.distance(goal) * 4.0).ceil() as usize;
        (0..=steps).all(|n| {
            let p = o.position.lerp(goal, n as f32 / steps.max(1) as f32);
            self.nav.cells[self.nav.map.index(p)] != 3
                && (o.visible(p) || self.nav.cells[self.nav.map.index(p)] < 2)
                && self
                    .nav
                    .nearby(o, p)
                    .all(|b| polygon_distance(p, &b.outline) > o.vehicle.size().y * 0.9)
        })
    }
    fn wall_on_axis(&self, o: &Observation, goal: Vec2) -> Option<Vec2> {
        if o.vehicle == VehicleKind::Scout {
            return None;
        }
        let axis = (goal - o.position).normalize_or_zero();
        let length = o.position.distance(goal);
        // First open the shot line, then widen it to fit the hull. Always aim
        // inside the actual visible polygon, not an empty path margin.
        for width in [0.15, o.vehicle.size().y * 0.95] {
            for n in 4..=(length * 4.0).ceil() as usize {
                let p = o.position + axis * (n as f32 * 0.25).min(length);
                if let Some(b) = self
                    .nav
                    .nearby(o, p)
                    .find(|b| b.breakable && polygon_distance(p, &b.outline) < width)
                {
                    let center =
                        b.outline.iter().copied().sum::<Vec2>() / b.outline.len().max(1) as f32;
                    let surface = b
                        .outline
                        .iter()
                        .zip(b.outline.iter().cycle().skip(1))
                        .take(b.outline.len())
                        .map(|(a, z)| {
                            let d = *z - *a;
                            *a + d
                                * ((p - *a).dot(d) / d.length_squared().max(0.001)).clamp(0.0, 1.0)
                        })
                        .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                        .unwrap_or(center);
                    return Some(surface.lerp(center, 0.25));
                }
            }
        }
        None
    }
    fn clearing_failed(&mut self, o: &Observation, station: Vec2, objective: Vec2) -> bool {
        let axis = (objective - station).normalize_or_zero();
        // Compare the same visible pieces of space. Total polygon area is not
        // a progress signal: opening an outer wall reveals more inner walls,
        // so that total can increase while a breach is making good progress.
        let samples: Vec<_> = (4..=(station.distance(objective) * 4.0).ceil() as usize)
            .flat_map(|n| {
                (-4..=4).map(move |side| {
                    station + axis * n as f32 * 0.25 + axis.perp() * side as f32 * 0.25
                })
            })
            .collect();
        let occupied: Vec<_> = samples
            .iter()
            .map(|p| {
                self.nav
                    .nearby(o, *p)
                    .any(|b| b.breakable && polygon_distance(*p, &b.outline) < 0.08)
            })
            .collect();
        let c = self.clearing.get_or_insert(Clearing {
            since: o.tick,
            started: o.tick,
            occupied: occupied.clone(),
            failed_shots: 0,
            reload: o.reload,
        });
        if c.occupied
            .iter()
            .zip(&occupied)
            .zip(&samples)
            .any(|((old, new), p)| *old && !*new && o.visible(*p))
        {
            c.since = o.tick;
            c.failed_shots = 0;
        }
        c.occupied = occupied;
        if o.reload > c.reload + 0.2 {
            c.failed_shots += 1;
        }
        c.reload = o.reload;
        c.failed_shots >= 8
            || o.tick.saturating_sub(c.since) > 3600
            || o.tick.saturating_sub(c.started) > 7200
    }
    fn traffic_task(&mut self, o: &Observation, intended: Option<Vec2>) -> Option<Task> {
        let ally = o.allied_vehicles.iter().min_by(|a, b| {
            a.position
                .distance_squared(o.position)
                .total_cmp(&b.position.distance_squared(o.position))
        });
        if let Some(p) = &mut self.passing {
            if let Some(a) = ally {
                p.last_ally = a.position;
                p.last_seen = o.tick;
            }
            let passed = ally.is_some_and(|a| {
                (a.position - p.bay).dot(p.axis) > 4.0 || a.position.distance(p.bay) > 16.0
            }) || o.tick.saturating_sub(p.last_seen) > 720;
            let still_blocking = o.position.distance(p.bay) < 0.8
                && ally.is_some_and(|a| a.position.distance(o.position) < 5.0);
            if passed || still_blocking || o.tick.saturating_sub(p.since) > 2400 {
                self.passing = None;
                self.route = None;
            } else {
                return Some(if o.position.distance(p.bay) < 0.8 {
                    Task::Wait
                } else {
                    Task::Yield(p.bay)
                });
            }
        }
        let ally = ally?;
        let goal: Vec2 = intended.unwrap_or_else(|| self.telemetry.goal.into());
        let axis = (goal - o.position).try_normalize().unwrap_or_else(|| {
            self.mission
                .as_ref()
                .map_or(Vec2::X, |m| (m.objective - o.position).normalize_or_zero())
        });
        let delta = ally.position - o.position;
        if delta.length() > 12.0 {
            return None;
        }
        let rank = |v| match v {
            VehicleKind::Tank => 2,
            VehicleKind::Mortar => 1,
            VehicleKind::Scout => 0,
        };
        let priority = rank(o.vehicle) > rank(ally.vehicle)
            || (o.vehicle == ally.vehicle && self.nav.map.team_slot == 0);
        if priority {
            if delta.perp_dot(axis).abs() > 3.5 {
                return None;
            }
            return (delta.dot(axis) > 0.0 && delta.length() < 4.5).then_some(Task::Wait);
        }
        let axis = self
            .teammate
            .and_then(|(_, _, v)| v.try_normalize())
            .unwrap_or_else(|| (o.position - ally.position).normalize_or_zero());
        if delta.perp_dot(axis).abs() > 3.5 {
            return None;
        }
        for bay in [
            o.position + axis + axis.perp() * 5.0,
            o.position + axis - axis.perp() * 5.0,
            o.position + axis * 12.0,
        ] {
            if self.nav.clearance(bay, o.vehicle) > 0.0
                && self.nav.corridor(o.position, bay, o.vehicle)
            {
                self.passing = Some(Passing {
                    bay,
                    axis,
                    since: o.tick,
                    last_ally: ally.position,
                    last_seen: o.tick,
                });
                return Some(Task::Yield(bay));
            }
        }
        // No passing bay: the follower leaves room; the leader is not ordered
        // to chase or stop ten metres behind it.
        (delta.dot(axis) > 0.0 && delta.length() < 4.5).then_some(Task::Wait)
    }
    fn recovery_task(&mut self, o: &Observation, wanted: Task) -> Option<Task> {
        if let Some(r) = &self.recovery {
            if o.tick < r.until && o.position.distance(r.point) > 0.5 {
                return Some(Task::Recover(r.point));
            }
            self.recovery = None;
            self.route = None;
            self.progress = None;
        }
        if let Some(enemy) = o.targets.iter().filter(|t| !t.mounted).min_by(|a, b| {
            a.position
                .distance_squared(o.position)
                .total_cmp(&b.position.distance_squared(o.position))
        }) && enemy.position.distance(o.position)
            < if o.vehicle == VehicleKind::Mortar {
                9.0
            } else {
                5.0
            }
        {
            let away = (o.position - enemy.position).normalize_or_zero();
            for angle in [0.0_f32, 0.6, -0.6] {
                let point = o.position + Vec2::from_angle(angle).rotate(away) * 4.0;
                if self.nav.corridor(o.position, point, o.vehicle) {
                    self.recovery = Some(Recovery {
                        point,
                        until: o.tick + 360,
                        reason: "visible enemy inside firing range",
                    });
                    self.progress = None;
                    return Some(Task::Recover(point));
                }
            }
        }
        if !wanted.moving()
            || self
                .route
                .as_ref()
                .is_some_and(|r| r.points.is_empty() && r.retry_at > o.tick)
        {
            self.progress = None;
            return None;
        }
        let p = self
            .progress
            .get_or_insert((o.tick, o.position, o.hull_angle));
        if o.position.distance(p.1) > 0.7 || angle_error(o.hull_angle, p.2).abs() > 0.4 {
            *p = (o.tick, o.position, o.hull_angle);
        }
        if o.tick.saturating_sub(p.0) < 600 {
            return None;
        }
        for heading in [
            o.hull_angle + std::f32::consts::PI,
            o.hull_angle + 1.2,
            o.hull_angle - 1.2,
        ] {
            let point = o.position + Vec2::from_angle(heading) * 3.0;
            if self.nav.corridor(o.position, point, o.vehicle) {
                self.recovery = Some(Recovery {
                    point,
                    until: o.tick + 360,
                    reason: "movement progress deadline",
                });
                self.telemetry.recoveries += 1;
                self.progress = None;
                return Some(Task::Recover(point));
            }
        }
        self.progress = Some((o.tick, o.position, o.hull_angle));
        None
    }
}
