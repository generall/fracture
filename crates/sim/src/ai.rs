//! Versioned controllers. Decisions accept only human-visible observations and
//! return the same input packets as a human multiplayer client.
mod arsenal;
mod mission;
mod navigation;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;
mod recovery;
mod siege;
mod task;
mod team;
use crate::{
    Ammunition, SpecialWeapon, Vec2, VehicleKind, Vision, level::Team, multiplayer::PlayerInput,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Reverse, collections::BinaryHeap};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub name: String,
    pub speed: f32,
    pub combat_range: f32,
    pub replan_seconds: f32,
    pub recover_seconds: f32,
    #[serde(default)]
    pub recovery_detour: bool,
    #[serde(default)]
    pub clear_fire_lane: bool,
    #[serde(default)]
    pub route_following: bool,
    #[serde(default)]
    pub advance_objective: bool,
    #[serde(default)]
    pub hunt: bool,
    #[serde(default)]
    pub adaptive_recovery: bool,
    #[serde(default)]
    pub bounded_clearing: bool,
    #[serde(default)]
    pub vehicle_tactics: bool,
    #[serde(default)]
    pub engagement_spacing: bool,
    #[serde(default)]
    pub productive_clearing: bool,
    #[serde(default)]
    pub objective_drive: bool,
    #[serde(default)]
    pub deliberate_aim: bool,
    #[serde(default)]
    pub tactical_routes: bool,
    #[serde(default)]
    pub threat_cost: u32,
    #[serde(default)]
    pub siege_tactics: bool,
    #[serde(default)]
    pub team_roles: bool,
    #[serde(default)]
    pub rolling_tracks: bool,
    #[serde(default)]
    pub prepared_routes: bool,
    #[serde(default)]
    pub safe_navigation: bool,
    #[serde(default)]
    pub raiding: bool,
    #[serde(default)]
    pub wide_routes: bool,
    #[serde(default)]
    pub supply_runs: bool,
    #[serde(default)]
    pub proactive_combat: bool,
    #[serde(default)]
    pub mobile_combat: bool,
    #[serde(default)]
    pub perimeter_routes: bool,
    /// Explicit task executive; legacy experiments remain replayable.
    #[serde(default)]
    pub task_control: bool,
    #[serde(default)]
    pub combined_arms: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            name: "v6-map-and-specials".into(),
            speed: 0.65,
            combat_range: 6.0,
            replan_seconds: 0.8,
            recover_seconds: 3.0,
            recovery_detour: true,
            clear_fire_lane: true,
            route_following: true,
            advance_objective: true,
            hunt: false,
            adaptive_recovery: true,
            bounded_clearing: true,
            vehicle_tactics: true,
            engagement_spacing: true,
            productive_clearing: true,
            objective_drive: true,
            deliberate_aim: true,
            tactical_routes: true,
            threat_cost: 20,
            siege_tactics: true,
            team_roles: true,
            rolling_tracks: true,
            prepared_routes: true,
            safe_navigation: true,
            raiding: false,
            wide_routes: false,
            supply_runs: false,
            proactive_combat: false,
            mobile_combat: false,
            perimeter_routes: false,
            task_control: false,
            combined_arms: false,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 80
            || !self.speed.is_finite()
            || !(0.1..=1.0).contains(&self.speed)
            || !self.combat_range.is_finite()
            || !(3.0..=24.0).contains(&self.combat_range)
            || !self.replan_seconds.is_finite()
            || !(0.1..=5.0).contains(&self.replan_seconds)
            || !self.recover_seconds.is_finite()
            || !(1.0..=10.0).contains(&self.recover_seconds)
            || self.threat_cost > 300
        {
            return Err("Invalid AI configuration".into());
        }
        Ok(())
    }
}

/// Public starting layout from the human map preview, never live hidden state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    pub size: Vec2,
    pub flags: Vec<(Team, Vec2)>,
    pub team: Team,
    /// Immutable starting cells: 1=driveable, 2=breakable, 3=water/cliff, 4=solid.
    #[serde(default)]
    pub layout: Vec<u8>,
    #[serde(default)]
    pub seed: u64,
    /// Public player slot within the team, also shown in the lobby/arena.
    #[serde(default)]
    pub team_slot: usize,
    /// Authored fixed defenses visible in the public starting-map preview.
    /// Destruction, capture, aiming and health are never refreshed while hidden.
    #[serde(default)]
    pub emplacements: Vec<Emplacement>,
    /// Public starting pickup markers, without hidden availability or type.
    #[serde(default)]
    pub supply_sites: Vec<Vec2>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Emplacement {
    pub team: Team,
    pub position: Vec2,
    pub facing: f32,
    pub arc: f32,
    pub range: f32,
}
impl Map {
    pub fn cell(&self) -> f32 {
        (self.size.max_element() / 160.0).max(0.75)
    }
    pub fn width(&self) -> usize {
        (self.size.x / self.cell()).ceil() as usize
    }
    pub fn height(&self) -> usize {
        (self.size.y / self.cell()).ceil() as usize
    }
    pub fn point(&self, i: usize) -> Vec2 {
        -self.size * 0.5
            + Vec2::new(
                (i % self.width()) as f32 + 0.5,
                (i / self.width()) as f32 + 0.5,
            ) * self.cell()
    }
    pub fn index(&self, point: Vec2) -> usize {
        let p = ((point + self.size * 0.5) / self.cell())
            .floor()
            .max(Vec2::ZERO);
        (p.y as usize).min(self.height() - 1) * self.width() + (p.x as usize).min(self.width() - 1)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Obstacle {
    pub outline: Vec<Vec2>,
    pub breakable: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Target {
    pub position: Vec2,
    pub mounted: bool,
    /// Chassis recognized from a normally visible figure, never a hidden unit
    /// or a shot-only reveal. The whole target passes through the sight delay.
    #[serde(default)]
    pub vehicle: Option<VehicleKind>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Supply {
    pub position: Vec2,
    pub weapon: SpecialWeapon,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SpecialState {
    pub weapon: SpecialWeapon,
    pub charges: u32,
    pub cooldown: f32,
    pub active: bool,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct AlliedVehicle {
    pub position: Vec2,
    pub vehicle: VehicleKind,
}
/// At least 300 ms between seeing new information and acting on it.
pub const REACTION_TICKS: u64 = 36;
pub const ARENA_TEAMS: [Team; 4] = [Team::Blue, Team::Red, Team::Blue, Team::Red];
pub const ARENA_PLAYERS: usize = 4;
pub fn player_seed(seed: u64, player: usize) -> u64 {
    seed.wrapping_add((player as u64).wrapping_mul(0x517cc1b727220a95))
}
pub const ROUND_SECONDS: u32 = 240;
pub const ROUND_TICKS: u32 = ROUND_SECONDS * 120;
/// Open-loop actuator plan for the next 100ms. It contains only ordinary
/// player inputs and cannot read another observation between decisions.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct InputPlan {
    pub input: PlayerInput,
    #[serde(default)]
    pub aim_path: Vec<Vec2>,
}
impl InputPlan {
    pub fn input_at(&self, step: usize) -> PlayerInput {
        let mut input = self.input;
        if !self.aim_path.is_empty() {
            let last = self.aim_path.len() - 1;
            input.aim = self.aim_path[step.min(last)];
            input.fire &= step >= last;
            input.special &= step >= last;
        }
        input
    }
    pub fn valid(&self) -> bool {
        self.input.valid()
            && self.aim_path.len() <= 12
            && (0..self.aim_path.len()).all(|i| self.input_at(i).valid())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Observation {
    pub tick: u64,
    pub position: Vec2,
    pub hull_angle: f32,
    pub turret_angle: f32,
    pub alive: bool,
    pub health: f32,
    pub reload: f32,
    /// Own aiming envelope, also shown by the human aiming reticle.
    #[serde(default)]
    pub aim_spread: f32,
    pub vehicle: VehicleKind,
    /// Sanitized geometry only; hidden tank IDs and visibility overrides are absent.
    pub regions: Vec<Vision>,
    #[serde(default)]
    pub special: Option<SpecialState>,
    #[serde(default)]
    pub supplies: Vec<Supply>,
    #[serde(default)]
    pub allies: Vec<Vec2>,
    #[serde(default)]
    pub allied_vehicles: Vec<AlliedVehicle>,
    pub obstacles: Vec<Obstacle>,
    pub targets: Vec<Target>,
    #[serde(default)]
    pub visible_ruins: Vec<Vec2>,
    /// Observed grid cells: 1=driveable, 3=water/cliff. Unseen cells are absent.
    pub terrain: Vec<(usize, u8)>,
}
impl Observation {
    pub fn visible(&self, point: Vec2) -> bool {
        self.regions.iter().any(|v| v.contains(point))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Telemetry {
    #[serde(default)]
    pub role: String,
    pub mode: String,
    pub goal: [f32; 2],
    pub waypoint: [f32; 2],
    pub route_cells: usize,
    pub known_cells: usize,
    pub recoveries: u32,
    pub replans: u32,
    pub moving: bool,
    #[serde(default)]
    pub decision: String,
    #[serde(default)]
    pub route_status: String,
    #[serde(default)]
    pub fire_reason: String,
    #[serde(default)]
    pub drive_reason: String,
}
struct CoverAttempt {
    point: Vec2,
    since: u64,
    best: u32,
    shots: u32,
    last_reload: f32,
}
#[derive(Clone, Copy)]
struct Breach {
    objective: Vec2,
    exit: Vec2,
    since: u64,
    shots: u32,
    last_reload: f32,
    drive_until: u64,
}
pub struct Controller {
    task_brain: Option<task::Brain>,
    config: Config,
    map: Map,
    memory: Vec<u8>,
    route: Vec<Vec2>,
    next_plan: u64,
    progress_at: Vec2,
    progress_tick: u64,
    progress_heading: f32,
    reverse_until: u64,
    last_seen: Option<(Vec2, u64)>,
    history: std::collections::VecDeque<(u64, Vec2)>,
    avoid: Vec<(Vec2, u64, isize)>,
    pub telemetry: Telemetry,
    perceptions: std::collections::VecDeque<Observation>,
    random: u64,
    deployment: Option<VehicleKind>,
    last_mine: Option<Vec2>,
    last_special: Option<SpecialWeapon>,
    special_ready_at: u64,
    clearing_anchor: Option<(Vec2, u64)>,
    vehicle: VehicleKind,
    scout_patrol: u32,
    cover_attempt: Option<CoverAttempt>,
    detour: Option<(Vec2, u64)>,
    tracked_target: Option<(Target, u64, Vec2)>,
    threats: Vec<(Target, u64)>,
    breach: Option<Breach>,
    support_goal: Option<(Vec2, u64)>,
    assault_approach: Option<Vec2>,
    known_ally: Option<(AlliedVehicle, u64, Vec2)>,
    yield_goal: Option<(Vec2, u64)>,
    retired_emplacements: Vec<bool>,
    prepared_costs: Vec<u32>,
    patrol_goal: Option<Vec2>,
    life_role: Option<&'static str>,
    supply_goal: Option<(Vec2, u64)>,
    visited_supplies: Vec<(Vec2, u64)>,
    travel_trail: std::collections::VecDeque<(u64, Vec2)>,
    supply_exit: Vec<Vec2>,
    supply_exit_until: u64,
    yield_reverse_until: u64,
    traffic_ally: Option<AlliedVehicle>,
    last_heading: Option<(u64, f32)>,
    yaw_rate: f32,
    flank_point: Option<Vec2>,
    flank_done: bool,
}
pub fn angle_error(target: f32, current: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}
impl Controller {
    pub fn plan_input(&mut self, input: PlayerInput) -> InputPlan {
        let aim_path = self
            .task_brain
            .as_mut()
            .map_or_else(Vec::new, |b| b.aim_path(input));
        InputPlan { input, aim_path }
    }
    pub fn set_patrol_goal(&mut self, goal: Vec2) {
        if self.patrol_goal.is_none_or(|p| p.distance(goal) > 0.1) {
            self.patrol_goal = Some(goal);
            self.next_plan = 0;
            self.breach = None;
        }
    }
    pub fn new(config: Config, map: Map) -> Self {
        let mut controller = Self {
            task_brain: config.task_control.then(|| {
                let mut brain = task::Brain::new(
                    map.clone(),
                    config.speed,
                    config.siege_tactics,
                    config.combined_arms,
                );
                brain.use_perimeter(config.perimeter_routes);
                brain
            }),
            prepared_costs: vec![],
            patrol_goal: None,
            life_role: None,
            supply_goal: None,
            visited_supplies: vec![],
            travel_trail: Default::default(),
            supply_exit: vec![],
            supply_exit_until: 0,
            yield_reverse_until: 0,
            traffic_ally: None,
            last_heading: None,
            yaw_rate: 0.0,
            flank_point: None,
            flank_done: false,
            retired_emplacements: vec![false; map.emplacements.len()],
            memory: if map.layout.len() == map.width() * map.height() {
                map.layout.clone()
            } else {
                vec![0; map.width() * map.height()]
            },
            random: map.seed ^ 0x9e3779b97f4a7c15 ^ if map.team == Team::Blue { 42 } else { 137 },
            deployment: None,
            last_mine: None,
            last_special: None,
            special_ready_at: 0,
            clearing_anchor: None,
            vehicle: VehicleKind::Tank,
            scout_patrol: 0,
            cover_attempt: None,
            detour: None,
            tracked_target: None,
            threats: vec![],
            breach: None,
            support_goal: None,
            assault_approach: None,
            known_ally: None,
            yield_goal: None,
            perceptions: Default::default(),
            map,
            config,
            route: vec![],
            next_plan: 0,
            progress_at: Vec2::ZERO,
            progress_tick: 0,
            progress_heading: 0.0,
            reverse_until: 0,
            last_seen: None,
            history: Default::default(),
            avoid: vec![],
            telemetry: Telemetry::default(),
        };
        controller.rebuild_prepared_costs();
        controller
    }
    fn rebuild_prepared_costs(&mut self) {
        self.prepared_costs = vec![0u32; self.map.width() * self.map.height()];
        if !self.config.prepared_routes {
            return;
        }
        for (i, gun) in self.map.emplacements.iter().enumerate() {
            if gun.team == self.map.team || self.retired_emplacements[i] {
                continue;
            }
            for (cell, cost) in self.prepared_costs.iter_mut().enumerate() {
                let delta = self.map.point(cell) - gun.position;
                if delta.length() < gun.range + 2.0
                    && angle_error(delta.to_angle(), gun.facing).abs()
                        < gun.arc.to_radians() * 0.5 + 0.08
                {
                    *cost = cost.saturating_add(2000);
                }
            }
        }
    }
    pub fn decide(&mut self, current: &Observation) -> PlayerInput {
        self.yaw_rate = if current.alive {
            self.last_heading
                .and_then(|(tick, heading)| {
                    let dt = current.tick.saturating_sub(tick) as f32 / 120.0;
                    (dt > 0.0 && dt <= 0.5)
                        .then(|| (angle_error(current.hull_angle, heading) / dt).clamp(-3.0, 3.0))
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        self.last_heading = current.alive.then_some((current.tick, current.hull_angle));
        // Sensor information is delayed as a whole. Own vehicle/inventory state
        // stays current; no enemy or scenery query bypasses the queue.
        if !current.alive {
            self.perceptions.clear();
            self.travel_trail.clear();
            self.supply_exit.clear();
            self.yield_reverse_until = 0;
        } else if self.config.raiding {
            if self
                .travel_trail
                .back()
                .is_none_or(|(_, p)| p.distance(current.position) > 0.5)
            {
                self.travel_trail
                    .push_back((current.tick, current.position));
            }
            while self
                .travel_trail
                .front()
                .is_some_and(|(tick, _)| current.tick.saturating_sub(*tick) > 7200)
            {
                self.travel_trail.pop_front();
            }
        }
        let carried = current.special.map(|s| s.weapon);
        if carried != self.last_special {
            self.last_special = carried;
            self.special_ready_at = current.tick + REACTION_TICKS;
            if self.config.raiding && carried == Some(SpecialWeapon::Rocket) {
                self.assault_approach = None;
                self.breach = None;
                // The coarse travel grid deliberately has more clearance than
                // a physical hull. Leave a narrow supply enclosure along our
                // own recent entrance, rather than sealing that exit as soon
                // as acquiring the weapon changes the strategic objective.
                if self.navigation_clearance(current.position, current.vehicle) < 0.0 {
                    self.supply_exit.clear();
                    for &(_, point) in self.travel_trail.iter().rev() {
                        if point.distance(current.position) < 1.0 {
                            continue;
                        }
                        self.supply_exit.push(point);
                        if point.distance(current.position) > 12.0
                            && self.navigation_clearance(point, current.vehicle) > 0.0
                        {
                            break;
                        }
                    }
                    self.supply_exit_until = current.tick + 3600;
                }
            }
        }
        self.perceptions.push_back(current.clone());
        let mut perceived = current.clone();
        perceived.regions.clear();
        perceived.targets.clear();
        perceived.visible_ruins.clear();
        perceived.obstacles.clear();
        perceived.terrain.clear();
        perceived.supplies.clear();
        perceived.allies.clear();
        perceived.allied_vehicles.clear();
        while self
            .perceptions
            .front()
            .is_some_and(|o| current.tick.saturating_sub(o.tick) >= REACTION_TICKS)
        {
            let old = self.perceptions.pop_front().unwrap();
            perceived.regions = old.regions;
            perceived.targets = old.targets;
            perceived.visible_ruins = old.visible_ruins;
            perceived.obstacles = old.obstacles;
            perceived.terrain = old.terrain;
            perceived.supplies = old.supplies;
            perceived.allies = old.allies;
            perceived.allied_vehicles = old.allied_vehicles;
        }
        // Keep the most recently processed perception between sensor updates.
        if let Some(brain) = &mut self.task_brain {
            let input = brain.decide(&perceived, self.patrol_goal);
            self.telemetry = brain.telemetry.clone();
            input
        } else {
            self.decide_perceived(&perceived)
        }
    }
    fn decide_perceived(&mut self, o: &Observation) -> PlayerInput {
        self.traffic_ally = o.allied_vehicles.first().copied();
        if self.config.raiding
            && self
                .traffic_ally
                .is_some_and(|ally| self.has_traffic_priority(o.vehicle, ally.vehicle))
        {
            // The priority vehicle keeps its route. Its escort yields locally;
            // continually routing both around each other makes a moving escort
            // drag the breacher back and forth across the same patch of road.
            self.traffic_ally = None;
        }
        if !o.alive {
            self.progress_at = o.position;
            self.progress_tick = o.tick;
            self.progress_heading = o.hull_angle;
            self.next_plan = 0;
            self.reverse_until = 0;
            self.history.clear();
            self.cover_attempt = None;
            self.detour = None;
            self.tracked_target = None;
            self.breach = None;
            self.support_goal = None;
            self.assault_approach = None;
            self.yield_goal = None;
            self.life_role = None;
            self.flank_done = false;
            self.supply_goal = None;
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
        if self.vehicle != o.vehicle {
            self.vehicle = o.vehicle;
            self.next_plan = 0;
            self.breach = None;
        }
        self.observe(o);
        let mut defense_changed = false;
        for (i, gun) in self.map.emplacements.iter().enumerate() {
            if !self.retired_emplacements[i]
                && gun.team != self.map.team
                && ((!self.config.raiding
                    && o.visible(gun.position)
                    && !o
                        .targets
                        .iter()
                        .any(|t| t.mounted && t.position.distance(gun.position) < 1.0))
                    || o.allies.iter().any(|p| p.distance(gun.position) < 0.5)
                    || o.visible_ruins
                        .iter()
                        .any(|p| p.distance(gun.position) < 1.0))
            {
                self.retired_emplacements[i] = true;
                defense_changed = true;
            }
        }
        if defense_changed {
            self.rebuild_prepared_costs();
            self.next_plan = 0;
        }
        self.remember_teammate(o);
        if let Some(breach) = self.breach
            && (o.position.distance(breach.exit) < 1.3
                || o.tick.saturating_sub(breach.since) > 7200)
        {
            self.breach = None;
            self.next_plan = 0;
        }
        if self.config.tactical_routes {
            self.threats
                .retain(|(_, tick)| o.tick.saturating_sub(*tick) < 1800);
            for &target in &o.targets {
                self.threats
                    .retain(|(old, _)| old.position.distance(target.position) > 4.0);
                self.threats.push((target, o.tick));
            }
        }
        let target = o
            .targets
            .iter()
            .filter(|t| {
                !(self.config.siege_tactics
                    && o.vehicle == VehicleKind::Mortar
                    && o.allies.iter().any(|p| p.distance(t.position) < 4.0))
            })
            .min_by(|a, b| {
                let scout_priority = |t: &&Target| {
                    self.config.raiding
                        && o.vehicle == VehicleKind::Scout
                        && t.vehicle == Some(VehicleKind::Mortar)
                };
                scout_priority(b).cmp(&scout_priority(a)).then_with(|| {
                    self.immediate_threat(o, **b)
                        .cmp(&self.immediate_threat(o, **a))
                        .then_with(|| {
                            a.position
                                .distance_squared(o.position)
                                .total_cmp(&b.position.distance_squared(o.position))
                        })
                })
            })
            .copied();
        let retaliating = target.is_some_and(|t| self.immediate_threat(o, t));
        if let Some(t) = target {
            self.last_seen = Some((t.position, o.tick));
        }
        let objective = self
            .map
            .flags
            .iter()
            .find(|(t, _)| *t == self.map.team.other())
            .map_or(o.position, |(_, p)| *p);
        let mut goal = if self.config.hunt {
            self.last_seen
                .filter(|(_, tick)| o.tick.saturating_sub(*tick) < 1200)
                .map_or(objective, |(p, _)| p)
        } else {
            objective
        };
        let role = self.team_role(o);
        let raiding = self.config.raiding && matches!(role, "breacher" | "independent");
        let flank = if raiding {
            self.flanking_goal(o, objective)
        } else {
            None
        };
        if let Some(point) = flank {
            goal = point;
        }
        self.telemetry.role = role.into();
        if let Some(team_goal) = self.team_goal(o, role, objective) {
            goal = team_goal;
        }
        if (role != "independent" || self.config.raiding)
            && let Some(orbit) = self.skirmish_goal(o, target)
        {
            goal = orbit;
        }
        if self.config.vehicle_tactics
            && !self.config.objective_drive
            && o.vehicle == VehicleKind::Scout
        {
            if o.position.distance(Vec2::from(self.telemetry.goal)) < 3.0 {
                self.scout_patrol += 1;
            }
            if o.position.distance(objective) < 20.0
                && self.corridor_clear(o.position, objective, o.vehicle)
            {
                goal = objective;
            } else {
                let center = target.map_or(objective, |t| t.position);
                let phase = self.scout_patrol + (o.tick / 2400) as u32;
                goal = (0..8)
                    .map(|n| {
                        center
                            + Vec2::from_angle((phase + n) as f32 * std::f32::consts::FRAC_PI_4)
                                * 14.0
                    })
                    .find(|p| {
                        p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(2.0)).all()
                            && self.memory[self.map.index(*p)] < 2
                    })
                    .unwrap_or(o.position);
            }
        }
        if raiding && flank.is_none() {
            goal = self.assault_goal(o, goal, objective, role);
        }
        if raiding
            && self.config.supply_runs
            && let Some(supply) = self.armament_goal(o, objective)
        {
            goal = supply;
        }
        if o.special.is_none()
            && o.vehicle != VehicleKind::Scout
            && let Some(supply) = o
                .supplies
                .iter()
                .filter(|p| {
                    (!self.config.raiding || (raiding && p.weapon != SpecialWeapon::Mine))
                        && p.position.distance(o.position) < 20.0
                        && (!self.config.objective_drive
                            || (p.position.distance(o.position)
                                < if self.config.raiding { 20.0 } else { 8.0 }
                                && p.position.distance(objective)
                                    < o.position.distance(objective) + 3.0))
                })
                .min_by(|a, b| {
                    a.position
                        .distance_squared(o.position)
                        .total_cmp(&b.position.distance_squared(o.position))
                })
        {
            goal = supply.position;
        }
        if let Some((point, until)) = self.detour {
            if o.tick < until && o.position.distance(point) > 2.0 {
                goal = point;
            } else {
                self.detour = None;
            }
        }
        if let Some(patrol) = self.patrol_goal {
            goal = patrol;
        }
        if self.config.raiding {
            let bounds = self.map.size * 0.5 - Vec2::splat(2.0);
            goal = goal.clamp(-bounds, bounds);
            if let Some(breach) = self.breach {
                goal = breach.objective;
            }
        }
        if !raiding {
            goal = self.assault_goal(o, goal, objective, role);
        }
        while self.supply_exit.first().is_some_and(|p| {
            p.distance(o.position)
                < if self.config.raiding && o.vehicle != VehicleKind::Scout && self.route.len() == 1
                {
                    0.25
                } else {
                    0.8
                }
        }) {
            self.supply_exit.remove(0);
        }
        if o.tick >= self.supply_exit_until {
            self.supply_exit.clear();
        }
        let leaving_supply = !self.supply_exit.is_empty();
        if leaving_supply {
            self.breach = None;
            goal = *self.supply_exit.last().unwrap();
            self.route = self.supply_exit.clone();
        }
        if Vec2::from(self.telemetry.goal).distance(goal) > self.map.cell() {
            self.next_plan = 0;
        }
        if leaving_supply {
            self.next_plan = o.tick + 120;
        }
        self.telemetry.goal = goal.to_array();
        if o.tick >= self.next_plan
            && !(self.config.siege_tactics && self.breach.is_some() && !self.config.raiding)
        {
            self.route = self.plan(o.position, goal);
            self.next_plan = o.tick + (self.config.replan_seconds * 120.0) as u64;
            self.telemetry.replans += 1;
        }
        while self
            .route
            .first()
            .is_some_and(|p| p.distance(o.position) < 0.8)
        {
            self.route.remove(0);
        }
        // Look ahead only through a clear corridor. Never cut across water or
        // the inside of a bend, even when the next few route cells are close.
        let mut waypoint = self
            .route
            .iter()
            .take_while(|p| p.distance(o.position) < 8.0)
            .filter(|p| self.corridor_clear(o.position, **p, o.vehicle))
            .last()
            .copied()
            .or_else(|| self.route.first().copied())
            .unwrap_or(if self.config.safe_navigation {
                o.position
            } else {
                goal
            });
        let unreachable = self.config.tactical_routes
            && self
                .route
                .last()
                .is_none_or(|p| p.distance(goal) > self.map.cell() * 2.0)
            && !self.corridor_clear(o.position, goal, o.vehicle);
        if unreachable && self.route.is_empty() && o.vehicle == VehicleKind::Scout {
            // A partial path is not permission to drive through the remaining
            // wall. Hold the reachable approach and cover a teammate's breach.
            waypoint = o.position;
        }
        self.telemetry.waypoint = waypoint.to_array();
        self.telemetry.route_cells = self.route.len();
        let mut tracks = self.steer(o, (waypoint - o.position).to_angle(), self.config.speed);
        if self.config.safe_navigation && self.breach.is_none() {
            tracks = self.navigation_controls(o, waypoint);
        }
        if unreachable && waypoint.distance(o.position) < 1.0 && o.vehicle == VehicleKind::Scout {
            tracks = [0.0; 2];
        }
        let mut clearing = self.obstruction(o, waypoint);
        if self.config.route_following {
            clearing = self.route_obstruction(o);
        }
        if self.config.clear_fire_lane
            && !self.config.objective_drive
            && let Some(t) = target
        {
            clearing = if self.config.siege_tactics && o.position.distance(objective) < 25.0 {
                clearing.or_else(|| self.fire_obstruction(o, t.position))
            } else if self.config.route_following {
                self.fire_obstruction(o, t.position)
                    .or_else(|| self.config.objective_drive.then_some(clearing).flatten())
            } else {
                self.fire_obstruction(o, t.position).or(clearing)
            };
        }
        let scout_blocked = self.config.vehicle_tactics
            && o.vehicle == VehicleKind::Scout
            && target.is_some_and(|t| self.fire_obstruction(o, t.position).is_some());
        if self.config.vehicle_tactics && o.vehicle == VehicleKind::Scout {
            clearing = None;
        }
        if leaving_supply {
            clearing = None;
        }
        let mut entering_breach = false;
        let mut flag_breach = false;
        if self.config.siege_tactics && o.vehicle != VehicleKind::Scout && !leaving_supply {
            if self.breach.is_none()
                && let Some(point) = clearing
            {
                self.breach = self.begin_breach(o, point);
            }
            if let Some(mut breach) = self.breach {
                if o.reload > 0.0 && breach.last_reload <= 0.0 {
                    breach.shots += 1;
                }
                breach.last_reload = o.reload;
                if (!self.config.raiding
                    || !o
                        .special
                        .is_some_and(|s| s.weapon == SpecialWeapon::Rocket && s.charges > 0))
                    && !(self.config.raiding && o.vehicle == VehicleKind::Mortar)
                    && o.tick >= breach.drive_until
                    && breach.shots
                        >= if o.vehicle == VehicleKind::Mortar {
                            2
                        } else {
                            3
                        }
                {
                    breach.shots = 0;
                    breach.drive_until = o.tick + 480;
                }
                self.breach = Some(breach);
                flag_breach = raiding
                    && breach.objective.distance(objective) < 1.0
                    && o.position.distance(objective) < 22.0;
                if flag_breach {
                    waypoint = objective;
                } else if !raiding {
                    waypoint = breach.exit;
                }
                self.telemetry.waypoint = waypoint.to_array();
                tracks = if raiding {
                    self.navigation_controls(o, waypoint)
                } else {
                    self.steer(o, (waypoint - o.position).to_angle(), self.config.speed)
                };
                clearing = if o.tick < breach.drive_until {
                    None
                } else if flag_breach {
                    self.breach_obstruction(o, objective)
                } else if raiding {
                    self.route_obstruction(o)
                } else {
                    self.breach_obstruction(o, breach.exit)
                };
                entering_breach = clearing.is_none();
            }
        }
        if retaliating {
            clearing = None;
            entering_breach = false;
            self.cover_attempt = None;
            self.clearing_anchor = None;
        }
        let ignored_cover = clearing.is_some_and(|point| {
            self.avoid
                .iter()
                .any(|(p, until, radius)| *radius > 0 && *until > o.tick && p.distance(point) < 2.5)
        });
        let abandoned = self.config.productive_clearing && self.watch_cover_progress(o, clearing);
        if ignored_cover || abandoned {
            clearing = None;
            self.breach = None;
            self.assault_approach = None;
        }
        if self.config.bounded_clearing && self.watch_clearing(o, clearing) {
            clearing = None;
            tracks = self.steer(o, o.hull_angle + 0.9, 0.25);
        }
        let mut aim = if self.config.clear_fire_lane {
            clearing.or(target.map(|t| t.position))
        } else {
            target.map(|t| t.position).or(clearing)
        }
        .or_else(|| {
            self.last_seen
                .filter(|(_, tick)| o.tick.saturating_sub(*tick) < 600)
                .map(|(p, _)| p)
        })
        .unwrap_or(waypoint);
        if flag_breach
            && o.vehicle == VehicleKind::Tank
            && clearing.is_some()
            && self.fire_obstruction(o, objective).is_some()
        {
            // A shell aimed through the flag hits the intervening visible
            // wall. Hold that axis while opening the channel, then widen its
            // sides only when the center line is already clear.
            aim = objective;
        }
        let predicted = target.map(|t| self.predict_target(o, t));
        if self.config.deliberate_aim && clearing.is_none() {
            aim = predicted.unwrap_or(aim);
        }
        let surveying = raiding
            && !retaliating
            && clearing.is_none()
            && !target.is_some_and(|t| t.mounted)
            && o.position.distance(goal) < 30.0
            && !self.corridor_clear(o.position, goal, o.vehicle);
        if surveying {
            aim = waypoint;
        }
        let mut mode = if entering_breach {
            "entering breach"
        } else {
            "driving"
        };
        if matches!(role, "support" | "guard") && o.position.distance(goal) < 2.8 {
            tracks = [0.0; 2];
            mode = "covering teammate";
        }
        if (!self.config.advance_objective
            || (self.config.vehicle_tactics
                && !self.config.objective_drive
                && o.vehicle == VehicleKind::Scout
                && !scout_blocked))
            && target.is_some_and(|t| t.position.distance(o.position) < self.config.combat_range)
        {
            tracks = [0.0; 2];
            mode = "combat";
        } else if clearing.is_some_and(|p| {
            p.distance(o.position)
                < if self.config.siege_tactics {
                    if o.vehicle == VehicleKind::Mortar {
                        13.0
                    } else {
                        if self.config.raiding
                            && o.special
                                .is_some_and(|s| s.weapon == SpecialWeapon::Rocket && s.charges > 0)
                        {
                            12.0
                        } else {
                            7.0
                        }
                    }
                } else {
                    5.0
                }
        }) {
            tracks = [0.0; 2];
            mode = "clearing";
        }
        if self.config.clear_fire_lane
            && clearing.is_some_and(|p| {
                p.distance(o.position)
                    <= if self.config.siege_tactics && o.vehicle == VehicleKind::Tank {
                        1.9
                    } else {
                        3.5
                    }
            })
        {
            tracks = [-0.25; 2];
            mode = "making firing space";
            if self.config.adaptive_recovery {
                let away = (o.position - clearing.unwrap()).normalize_or_zero();
                let candidate = [away, Vec2::new(-away.y, away.x), Vec2::new(away.y, -away.x)]
                    .into_iter()
                    .find(|d| self.observed_clear(o, o.position + *d * 1.5))
                    .unwrap_or(away);
                tracks = self.steer(o, candidate.to_angle(), 0.30);
            }
        }
        if o.vehicle == VehicleKind::Mortar
            && (!raiding || clearing.is_some() || target.is_some_and(|t| t.mounted))
            && let Some(point) = (self.config.siege_tactics.then_some(clearing).flatten())
                .or(target.map(|t| t.position))
        {
            let range = point.distance(o.position);
            if (6.5..=35.0).contains(&range) {
                aim = if self.config.deliberate_aim && clearing.is_none() {
                    predicted.unwrap_or(point)
                } else {
                    point
                };
                tracks = self.steer(o, (aim - o.position).to_angle(), 0.0);
                mode = "mortar aiming";
            }
        }
        if self.config.engagement_spacing {
            let minimum = if o.vehicle == VehicleKind::Mortar {
                8.0
            } else {
                5.5
            };
            let maximum = if o.vehicle == VehicleKind::Mortar {
                32.0
            } else {
                self.config.combat_range.clamp(7.5, 18.0)
            };
            let flag_run = self.config.advance_objective
                && ((o.position.distance(objective) < 7.5
                    && self.corridor_clear(o.position, objective, o.vehicle))
                    || (entering_breach && o.position.distance(objective) < 25.0));
            let danger = target
                .filter(|t| {
                    !flag_run
                        && t.position.distance(o.position) < if raiding { 3.0 } else { minimum }
                })
                .map(|t| t.position)
                .or(clearing.filter(|p| {
                    p.distance(o.position)
                        < if o.vehicle == VehicleKind::Mortar {
                            if self.config.raiding { 6.5 } else { 7.5 }
                        } else if self.config.siege_tactics {
                            2.1
                        } else {
                            4.2
                        }
                }));
            if let Some(point) = danger {
                let spacing = if self.config.siege_tactics
                    && o.vehicle == VehicleKind::Tank
                    && clearing.is_some_and(|p| p.distance(point) < 0.1)
                {
                    3.0
                } else {
                    minimum + 1.0
                };
                tracks = self.open_range(o, point, spacing);
                mode = "opening range";
            } else if !flag_run
                && (!raiding || target.is_some_and(|t| t.mounted))
                && !(self.config.objective_drive && o.vehicle == VehicleKind::Scout)
                && target.is_some_and(|t| {
                    t.position.distance(o.position) <= maximum
                        && (o.vehicle == VehicleKind::Mortar || !Self::blocked_shot(o, t.position))
                })
            {
                if self.config.siege_tactics && o.vehicle == VehicleKind::Mortar {
                    // A mortar aims with its hull. A combat hold must retain
                    // the aiming turn instead of cancelling it every update.
                    tracks = self.steer(o, (aim - o.position).to_angle(), 0.0);
                    mode = "mortar aiming";
                } else {
                    tracks = [0.0; 2];
                    mode = "combat";
                }
            }
        }
        let ally_blocked = self.config.engagement_spacing
            && (target.is_some() || clearing.is_some())
            && if self.config.siege_tactics && o.vehicle == VehicleKind::Mortar {
                o.allies.iter().any(|p| p.distance(aim) < 4.0)
            } else {
                let d = aim - o.position;
                o.allies.iter().any(|p| {
                    let along = (*p - o.position).dot(d) / d.length_squared().max(0.01);
                    (0.05..0.98).contains(&along) && p.distance(o.position + d * along) < 1.3
                })
            };
        if ally_blocked && self.config.siege_tactics && o.vehicle == VehicleKind::Mortar {
            tracks = self.steer(o, (aim - o.position).to_angle(), 0.0);
            mode = "waiting for clear impact";
        } else if ally_blocked
            && mode != "opening range"
            && !(role == "breacher" && self.breach.is_some())
        {
            let side = (aim - o.position).normalize_or_zero().perp();
            if let Some(point) = [side, -side]
                .into_iter()
                .map(|d| o.position + d * 3.5)
                .find(|p| {
                    self.observed_clear(o, *p) && self.corridor_clear(o.position, *p, o.vehicle)
                })
            {
                tracks = self.steer(o, (point - o.position).to_angle(), 0.35);
                mode = "clearing ally lane";
            }
        }
        let muzzle_blocked = self.config.siege_tactics
            && o.vehicle == VehicleKind::Tank
            && !(flag_breach && clearing.is_none() && o.position.distance(objective) < 5.0)
            && (target.is_some() || clearing.is_some())
            && Self::blocked_shot(o, o.position + Vec2::from_angle(o.turret_angle) * 1.7);
        if muzzle_blocked {
            tracks = self.open_range(o, aim, 3.5);
            mode = "clearing muzzle";
        }
        if retaliating && !muzzle_blocked && o.position.distance(objective) > 3.5 {
            let range = aim.distance(o.position);
            if o.vehicle == VehicleKind::Tank
                && range > 6.5
                && target.is_some_and(|t| {
                    self.config.mobile_combat || t.vehicle == Some(VehicleKind::Mortar)
                })
            {
                let enemy = target.unwrap().position;
                let radial = (o.position - enemy).normalize_or_zero();
                let preferred = if self.map.team_slot.is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                if let Some(point) = [preferred, -preferred]
                    .into_iter()
                    .map(|side| enemy + Vec2::from_angle(side * 0.75).rotate(radial) * 9.0)
                    .find(|p| {
                        self.observed_clear(o, *p) && self.corridor_clear(o.position, *p, o.vehicle)
                    })
                {
                    let heading = (point - o.position).to_angle();
                    let ahead = o.position + Vec2::from_angle(o.hull_angle) * 4.0;
                    tracks = if angle_error(heading, o.hull_angle).abs() < 1.0
                        && self.corridor_clear(o.position, ahead, o.vehicle)
                    {
                        if self.config.mobile_combat {
                            let error = angle_error(heading, o.hull_angle);
                            let forward = 0.85 * error.cos().max(0.0).powi(2);
                            let turn = self.tracked_turn(o, error);
                            [forward + turn, forward - turn].map(|v| v.clamp(-1.0, 1.0))
                        } else {
                            self.steer(o, heading, 0.85)
                        }
                    } else {
                        self.navigation_controls(o, point)
                    };
                }
                mode = if self.config.mobile_combat {
                    "mobile combat"
                } else {
                    "flanking mortar"
                };
            } else {
                tracks = if range
                    < if o.vehicle == VehicleKind::Mortar {
                        10.0
                    } else {
                        6.5
                    } {
                    self.open_range(
                        o,
                        aim,
                        if o.vehicle == VehicleKind::Mortar {
                            13.0
                        } else {
                            9.0
                        },
                    )
                } else if o.vehicle == VehicleKind::Mortar {
                    self.steer(o, (aim - o.position).to_angle(), 0.0)
                } else {
                    [0.0; 2]
                };
                mode = if self.config.proactive_combat && o.health >= 95.0 {
                    "engaging vehicle"
                } else {
                    "returning fire"
                };
            }
        }
        if let Some(yield_tracks) = self.team_traffic(o, role, tracks) {
            tracks = yield_tracks;
            mode = "yielding to teammate";
            self.history.clear();
            self.progress_tick = o.tick;
        }
        (tracks, mode) = self.movement_recovery(o, waypoint, tracks, mode);
        if leaving_supply {
            tracks = self.short_drive(o, self.supply_exit[0], 0.35);
            mode = "leaving supply";
        }
        // Inside a breached flag room, use visible geometry for the last few
        // metres. Coarse cell clearance and a blocked barrel must not send a
        // tank back outside when its hull already has a clear path to the flag.
        let approaching_flag = self.config.raiding
            && o.position.distance(objective) < 4.0
            && (0..=20).all(|n| {
                let point = o.position.lerp(objective, n as f32 / 20.0);
                self.memory[self.map.index(point)] != 3
                    && o.obstacles
                        .iter()
                        .all(|b| polygon_distance(point, &b.outline) > o.vehicle.size().y * 0.85)
            });
        if approaching_flag {
            tracks = self.short_drive(o, objective, 0.28);
            mode = "capturing flag";
            self.telemetry.waypoint = objective.to_array();
        }
        let special = o.tick >= self.special_ready_at
            && o.vehicle != VehicleKind::Scout
            && o.special.is_some_and(|s| {
                if s.charges == 0 || s.cooldown > 0.0 || s.active {
                    return false;
                }
                if s.weapon == SpecialWeapon::Mine {
                    return self.last_mine.is_none_or(|p| p.distance(o.position) > 8.0)
                        && (self.config.raiding
                            || o.position.distance(objective) < 25.0
                            || target.is_some_and(|t| t.position.distance(o.position) < 18.0));
                }
                if self.config.raiding
                    && !retaliating
                    && (clearing.is_none() || o.position.distance(objective) > 30.0)
                {
                    return false;
                }
                let range = aim.distance(o.position);
                let aligned =
                    angle_error((aim - o.position).to_angle(), o.turret_angle).abs() < 0.07;
                let useful = target.is_some() || clearing.is_some();
                useful
                    && aligned
                    && range > 4.0
                    && range < 30.0
                    && (s.weapon != SpecialWeapon::Rocket || range > 7.0)
                    && !o.allies.iter().any(|p| {
                        let delta = aim - o.position;
                        let fraction = ((*p - o.position).dot(delta)
                            / delta.length_squared().max(0.01))
                        .clamp(0.0, 1.0);
                        p.distance(o.position + delta * fraction)
                            < if s.weapon == SpecialWeapon::Rocket {
                                3.0
                            } else {
                                1.5
                            }
                    })
            });
        if special && o.special.is_some_and(|s| s.weapon == SpecialWeapon::Mine) {
            self.last_mine = Some(o.position);
        }
        self.telemetry.moving = tracks != [0.0; 2];
        self.telemetry.mode = mode.into();
        let acquired = (target.is_some() || clearing.is_some()) && !surveying;
        let aimed = angle_error((aim - o.position).to_angle(), o.turret_angle).abs() < 0.07;
        // Let the same reticle a human sees settle. Firing immediately after a
        // turn wastes shells, particularly with the mortar's landing scatter.
        let settled = !self.config.deliberate_aim
            || o.vehicle == VehicleKind::Scout
            || (o.vehicle == VehicleKind::Tank
                && clearing.is_none()
                && target.is_some_and(|t| !t.mounted || o.health < 85.0))
            || o.aim_spread
                < if o.vehicle == VehicleKind::Mortar {
                    3.0_f32.to_radians()
                } else {
                    (1.5 / aim.distance(o.position).max(1.0))
                        .atan()
                        .clamp(4.0_f32.to_radians(), 12.0_f32.to_radians())
                };
        PlayerInput {
            tracks,
            aim,
            special,
            ammunition: if self.config.siege_tactics {
                // AP's concentrated impact breaches masonry; mortar/scout
                // retain their actual fixed main round. Keeping the selection
                // stable also avoids restarting reload on every target change.
                if o.vehicle == VehicleKind::Mortar {
                    Ammunition::HighExplosive
                } else {
                    Ammunition::ArmorPiercing
                }
            } else if target.is_some() && !(self.config.clear_fire_lane && clearing.is_some()) {
                Ammunition::ArmorPiercing
            } else {
                Ammunition::HighExplosive
            },
            fire: acquired
                && !muzzle_blocked
                && !scout_blocked
                && !ally_blocked
                && !(ignored_cover || abandoned)
                && aimed
                && settled
                && o.reload <= 0.0
                && aim.distance(o.position)
                    > if o.vehicle == VehicleKind::Mortar {
                        6.5
                    } else if self.config.siege_tactics && clearing.is_some() {
                        1.9
                    } else {
                        3.3
                    }
                && (o.vehicle != VehicleKind::Mortar || aim.distance(o.position) <= 36.0),
            ..Default::default()
        }
    }
    fn begin_breach(&self, o: &Observation, point: Vec2) -> Option<Breach> {
        let direction = (point - o.position).normalize_or_zero();
        let radius = o.vehicle.size().y * 1.1;
        [0.0, -1.5, 1.5, -3.0, 3.0].into_iter().find_map(|offset| {
            let exit = point + direction * 3.0 + direction.perp() * offset;
            let steps = (o.position.distance(exit) * 3.0).ceil() as usize;
            let possible = (1..=steps).all(|n| {
                let p = o.position.lerp(exit, n as f32 / steps.max(1) as f32);
                self.memory[self.map.index(p)] < 3
                    && o.obstacles
                        .iter()
                        .filter(|b| !b.breakable)
                        .all(|b| polygon_distance(p, &b.outline) > radius)
            });
            possible.then_some(Breach {
                objective: self.telemetry.goal.into(),
                exit,
                since: o.tick,
                shots: 0,
                last_reload: o.reload,
                drive_until: 0,
            })
        })
    }
    fn breach_obstruction(&self, o: &Observation, exit: Vec2) -> Option<Vec2> {
        let steps = (exit.distance(o.position) * 4.0).ceil() as usize;
        for n in 0..=steps {
            let p = o.position.lerp(exit, n as f32 / steps.max(1) as f32);
            if let Some(body) = o.obstacles.iter().find(|b| {
                b.breakable && polygon_distance(p, &b.outline) < o.vehicle.size().y * 1.15
            }) {
                let center = body.outline.iter().copied().sum::<Vec2>() / body.outline.len() as f32;
                let closest = body
                    .outline
                    .iter()
                    .zip(body.outline.iter().cycle().skip(1))
                    .take(body.outline.len())
                    .map(|(a, b)| {
                        let d = *b - *a;
                        *a + d * ((p - *a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0)
                    })
                    .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                    .unwrap_or(center);
                return Some(closest.lerp(center, 0.25));
            }
        }
        None
    }
    fn immediate_threat(&self, o: &Observation, target: Target) -> bool {
        self.config.raiding
            && o.vehicle != VehicleKind::Scout
            && (self.config.proactive_combat || o.health < 95.0)
            && !target.mounted
            && target.position.distance(o.position) < self.config.combat_range
            && !Self::blocked_shot(o, target.position)
    }
    fn predict_target(&mut self, o: &Observation, target: Target) -> Vec2 {
        let mut velocity = Vec2::ZERO;
        if !target.mounted
            && let Some((previous, tick, previous_velocity)) = self.tracked_target
        {
            let dt = o.tick.saturating_sub(tick) as f32 / 120.0;
            let moved = target.position - previous.position;
            if !previous.mounted
                && previous.vehicle == target.vehicle
                && dt > 0.0
                && dt <= 0.3
                && moved.length() <= 12.0 * dt
            {
                velocity = previous_velocity.lerp(moved / dt, 0.5);
            }
        }
        self.tracked_target = Some((target, o.tick, velocity));
        let range = target.position.distance(o.position);
        let flight = if o.vehicle == VehicleKind::Mortar {
            0.8 + range / 24.0
        } else {
            range / 260.0
        };
        // Only consecutive delayed sightings are extrapolated. Authoritative
        // velocity and hidden positions never enter this calculation.
        target.position + velocity * (REACTION_TICKS as f32 / 120.0 + flight).min(2.0)
    }
    fn blocked_shot(o: &Observation, target: Vec2) -> bool {
        let d = target - o.position;
        o.obstacles.iter().any(|b| {
            b.outline
                .iter()
                .zip(b.outline.iter().cycle().skip(1))
                .take(b.outline.len())
                .any(|(a, z)| {
                    let edge = *z - *a;
                    let denominator = d.perp_dot(edge);
                    if denominator.abs() < 1e-6 {
                        return false;
                    }
                    let t = (*a - o.position).perp_dot(edge) / denominator;
                    let u = (*a - o.position).perp_dot(d) / denominator;
                    (0.02..0.98).contains(&t) && (0.0..=1.0).contains(&u)
                })
        })
    }
    fn open_range(&self, o: &Observation, threat: Vec2, range: f32) -> [f32; 2] {
        let mut away = (o.position - threat).normalize_or_zero();
        if away == Vec2::ZERO {
            away = -Vec2::from_angle(o.hull_angle);
        }
        let goal = [0.0, -0.65, 0.65, -1.25, 1.25]
            .into_iter()
            .map(|angle| {
                let direction = Vec2::from_angle(angle).rotate(away);
                o.position + direction * (range - o.position.distance(threat)).clamp(2.0, 4.0)
            })
            .find(|point| {
                self.observed_clear(o, *point)
                    && self.corridor_clear(o.position, *point, o.vehicle)
                    && o.allies.iter().all(|p| p.distance(*point) > 2.5)
            })
            .or_else(|| {
                [away, away.perp(), -away.perp()]
                    .into_iter()
                    .map(|d| o.position + d * 3.0)
                    .find(|p| {
                        self.observed_clear(o, *p)
                            && p.distance(threat) > o.position.distance(threat) + 0.5
                    })
            });
        let Some(goal) = goal else {
            if self.config.raiding && o.vehicle != VehicleKind::Scout {
                let point = o.position + away * 3.0;
                if (0..=12).all(|n| {
                    let p = o.position.lerp(point, n as f32 / 12.0);
                    self.memory[self.map.index(p)] != 3
                        && o.obstacles
                            .iter()
                            .filter(|b| !b.breakable)
                            .all(|b| polygon_distance(p, &b.outline) > o.vehicle.size().y * 0.9)
                }) {
                    return self.short_drive(o, point, 0.3);
                }
            }
            if self.config.safe_navigation {
                return self.navigation_controls(o, o.position + away * 4.0);
            }
            if self.config.siege_tactics {
                // A clear path can be absent because the vehicle is standing
                // among its own breach debris. Try a physical escape instead
                // of returning zero input forever beside the wall.
                let ahead = (threat - o.position).dot(Vec2::from_angle(o.hull_angle)) > 0.0;
                let sign = if ahead { -1.0 } else { 1.0 };
                let escape = o.position + Vec2::from_angle(o.hull_angle) * sign * 2.0;
                if self.memory[self.map.index(escape)] < 3 {
                    return [sign * 0.4; 2];
                }
                return [0.12, -0.12];
            }
            return [0.0; 2];
        };
        let heading = (goal - o.position).to_angle();
        let reverse = angle_error(heading, o.hull_angle).abs() > std::f32::consts::FRAC_PI_2;
        let desired = heading + if reverse { std::f32::consts::PI } else { 0.0 };
        let error = angle_error(desired, o.hull_angle);
        if o.vehicle == VehicleKind::Scout {
            let throttle = if reverse { -0.45 } else { 0.45 };
            let steering = (error * 1.2 * if reverse { -1.0 } else { 1.0 }).clamp(-1.0, 1.0);
            [(throttle + steering) * 0.5, (throttle - steering) * 0.5]
        } else {
            let forward = if error.abs() < 0.7 {
                if reverse { -0.4 } else { 0.4 }
            } else {
                0.0
            };
            let turn = if self.config.raiding {
                self.tracked_turn(o, error)
            } else {
                (error * 0.13).clamp(-0.12, 0.12)
            };
            [forward + turn, forward - turn]
        }
    }
    fn watch_cover_progress(&mut self, o: &Observation, clearing: Option<Vec2>) -> bool {
        let Some(point) = clearing else {
            return false;
        };
        // Compare only visible cover near the impact. A human can see a breach
        // opening; no material health, hidden shapes or damage counters are used.
        let nearby: Vec<_> = o
            .obstacles
            .iter()
            .filter(|b| polygon_distance(point, &b.outline) < 2.0)
            .collect();
        let occupied = (-4..=4)
            .flat_map(|y| (-4..=4).map(move |x| point + Vec2::new(x as f32, y as f32) * 0.25))
            .filter(|p| {
                nearby
                    .iter()
                    .any(|b| polygon_distance(*p, &b.outline) < 0.12)
            })
            .count() as u32;
        if self
            .cover_attempt
            .as_ref()
            .is_none_or(|a| a.point.distance(point) > 2.0)
        {
            self.cover_attempt = Some(CoverAttempt {
                point,
                since: o.tick,
                best: occupied,
                shots: 0,
                last_reload: o.reload,
            });
        }
        let attempt = self.cover_attempt.as_mut().unwrap();
        if occupied < attempt.best {
            attempt.best = occupied;
            attempt.since = o.tick;
            attempt.shots = 0;
        }
        if o.reload > 0.0 && attempt.last_reload <= 0.0 {
            attempt.shots += 1;
        }
        attempt.last_reload = o.reload;
        if o.tick.saturating_sub(attempt.since) < 1440 || attempt.shots < 6 {
            return false;
        }
        self.avoid.push((point, o.tick + 5400, 2));
        let away = (o.position - point).normalize_or_zero();
        let flank = [away.perp(), -away.perp(), away]
            .into_iter()
            .map(|d| o.position + d * 7.0)
            .find(|p| self.observed_clear(o, *p))
            .unwrap_or(o.position);
        self.detour = Some((flank, o.tick + 1200));
        self.cover_attempt = None;
        self.next_plan = 0;
        self.telemetry.recoveries += 1;
        true
    }
    fn watch_clearing(&mut self, o: &Observation, clearing: Option<Vec2>) -> bool {
        if o.vehicle != VehicleKind::Scout {
            return false;
        }
        let Some(point) = clearing else {
            return false;
        };
        if self
            .clearing_anchor
            .is_none_or(|(p, _)| p.distance(point) > 2.0)
        {
            self.clearing_anchor = Some((point, o.tick));
        }
        let limit = if o.vehicle == VehicleKind::Scout {
            720
        } else {
            1440
        };
        if self
            .clearing_anchor
            .is_some_and(|(_, tick)| o.tick.saturating_sub(tick) >= limit)
        {
            self.avoid.push((point, o.tick + 3600, 2));
            self.next_plan = 0;
            self.clearing_anchor = None;
            self.telemetry.recoveries += 1;
            return true;
        }
        false
    }
    fn steer(&self, o: &Observation, heading: f32, speed: f32) -> [f32; 2] {
        let error = angle_error(heading, o.hull_angle);
        if o.vehicle == VehicleKind::Scout {
            let throttle = speed.min(0.65) * (1.0 - error.abs().min(1.0) * 0.65);
            let steering = (error * 1.2).clamp(-1.0, 1.0);
            return [(throttle + steering) * 0.5, (throttle - steering) * 0.5];
        }
        let turn_limit = if self.config.siege_tactics && o.vehicle == VehicleKind::Mortar {
            0.23
        } else {
            0.12
        };
        let mut turn = (error * turn_limit / 0.92).clamp(-turn_limit, turn_limit);
        if self.config.siege_tactics && o.vehicle == VehicleKind::Mortar && speed == 0.0 {
            // Tiny proportional requests cannot overcome hull friction. Finish
            // the final aiming correction with a bounded input and deadband.
            turn = if error.abs() > 0.025 {
                turn.signum() * turn.abs().max(0.06)
            } else {
                0.0
            };
        }
        if self.config.raiding {
            turn = self.tracked_turn(o, error);
        }
        let forward = if self.config.rolling_tracks {
            speed * error.cos().max(0.0).powi(2)
        } else if error.abs() < 0.6 {
            speed * (1.0 - error.abs())
        } else {
            0.0
        };
        [forward + turn, forward - turn].map(|t| t.clamp(-1.0, 1.0))
    }
    fn corridor_clear(&self, start: Vec2, end: Vec2, vehicle: VehicleKind) -> bool {
        if self.config.safe_navigation {
            return self.navigation_corridor(start, end, vehicle);
        }
        let delta = end - start;
        let side = delta.normalize_or_zero().perp() * vehicle.size().y * 0.9;
        let steps = (delta.length() / self.map.cell() * 2.0).ceil() as usize;
        (0..=steps).all(|i| {
            let p = start + delta * (i as f32 / steps.max(1) as f32);
            if self.config.prepared_routes {
                // Lookahead must preserve the same bank clearance as A*.
                // Otherwise it immediately cuts the safer route's river bend.
                let at = self.map.index(p);
                let w = self.map.width();
                let clearance = 1.36 * vehicle.size().max_element() + self.map.cell() * 0.71;
                for dy in -3isize..=3 {
                    for dx in -3isize..=3 {
                        let x = (at % w) as isize + dx;
                        let y = (at / w) as isize + dy;
                        if x < 0 || y < 0 || x >= w as isize || y >= self.map.height() as isize {
                            continue;
                        }
                        let cell = y as usize * w + x as usize;
                        if self.memory[cell] == 3 && self.map.point(cell).distance(p) < clearance {
                            return false;
                        }
                    }
                }
            }
            [p, p + side, p - side]
                .iter()
                .all(|q| self.memory[self.map.index(*q)] < 2)
        })
    }
    fn observed_clear(&self, o: &Observation, point: Vec2) -> bool {
        point
            .abs()
            .cmplt(self.map.size * 0.5 - Vec2::splat(1.3))
            .all()
            && self.memory[self.map.index(point)] < 2
            && o.obstacles
                .iter()
                .all(|b| polygon_distance(point, &b.outline) > 1.0)
    }
    fn observe(&mut self, o: &Observation) {
        for &(i, value) in &o.terrain {
            self.memory[i] = value;
        }
        let cell = self.map.cell();
        for obstacle in &o.obstacles {
            let lo = obstacle
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::INFINITY), Vec2::min);
            let hi = obstacle
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
            let a = self.map.index(lo - Vec2::splat(cell));
            let b = self.map.index(hi + Vec2::splat(cell));
            for y in a / self.map.width()..=b / self.map.width() {
                for x in a % self.map.width()..=b % self.map.width() {
                    let i = y * self.map.width() + x;
                    if polygon_distance(self.map.point(i), &obstacle.outline) < cell * 0.6 {
                        self.memory[i] = if obstacle.breakable { 2 } else { 4 };
                    }
                }
            }
        }
        self.telemetry.known_cells = self.memory.iter().filter(|&&v| v > 0).count();
    }
    fn obstruction(&self, o: &Observation, waypoint: Vec2) -> Option<Vec2> {
        let direction = (waypoint - o.position).normalize_or_zero();
        (1..=12)
            .map(|n| o.position + direction * n as f32 * 0.5)
            .find_map(|p| {
                o.obstacles
                    .iter()
                    .find(|b| b.breakable && polygon_distance(p, &b.outline) < 0.9)
                    .map(|_| p)
            })
    }
    fn fire_obstruction(&self, o: &Observation, target: Vec2) -> Option<Vec2> {
        let delta = target - o.position;
        let distance = delta.length();
        let direction = delta.normalize_or_zero();
        (4..(distance * 4.0) as usize).find_map(|n| {
            let p = o.position + direction * n as f32 * 0.25;
            o.obstacles
                .iter()
                .find(|b| polygon_distance(p, &b.outline) < 0.12)
                .and_then(|b| b.breakable.then_some(p))
        })
    }
    fn route_obstruction(&self, o: &Observation) -> Option<Vec2> {
        let mut start = o.position;
        let mut remaining = if self.config.siege_tactics
            && (o.vehicle == VehicleKind::Mortar
                || (self.config.raiding
                    && o.special
                        .is_some_and(|s| s.weapon == SpecialWeapon::Rocket && s.charges > 0)))
        {
            14.0
        } else {
            6.0
        };
        for &waypoint in &self.route {
            let delta = waypoint - start;
            let length = delta.length().min(remaining);
            let dir = delta.normalize_or_zero();
            for n in 0..=(length * 4.0).ceil() as usize {
                let p = start + dir * (n as f32 * 0.25).min(length);
                if let Some(b) = o
                    .obstacles
                    .iter()
                    .find(|b| polygon_distance(p, &b.outline) < 0.95)
                {
                    if !b.breakable {
                        return None;
                    }
                    let center = b.outline.iter().copied().sum::<Vec2>() / b.outline.len() as f32;
                    // The path's swept hull can touch a side wall. Shoot at its
                    // actual visible surface rather than an empty point beside it.
                    let closest = b
                        .outline
                        .iter()
                        .zip(b.outline.iter().cycle().skip(1))
                        .take(b.outline.len())
                        .map(|(a, z)| {
                            let d = *z - *a;
                            *a + d
                                * ((p - *a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0)
                        })
                        .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
                        .unwrap_or(center);
                    return Some(closest.lerp(center, 0.25));
                }
            }
            remaining -= length;
            if remaining <= 0.01 {
                break;
            }
            start = waypoint;
        }
        None
    }
    fn plan(&self, start: Vec2, goal: Vec2) -> Vec<Vec2> {
        self.plan_with_cost(start, goal).0
    }
    fn plan_with_cost(&self, start: Vec2, goal: Vec2) -> (Vec<Vec2>, u32) {
        let w = self.map.width();
        let h = self.map.height();
        let cell = self.map.cell();
        let mut costs = vec![10u32; w * h];
        for (i, &v) in self.memory.iter().enumerate() {
            let p = self.map.point(i);
            if p.abs().cmpgt(self.map.size * 0.5 - Vec2::splat(1.4)).any() {
                costs[i] = u32::MAX;
            }
            if v < 2 {
                continue;
            }
            for dy in -2isize..=2 {
                for dx in -2isize..=2 {
                    let x = (i % w) as isize + dx;
                    let y = (i / w) as isize + dy;
                    if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                        continue;
                    }
                    let at = y as usize * w + x as usize;
                    // Reserve room for the complete rotating hull, including
                    // the uncertainty of a cell sampled at its center. Merely
                    // keeping the center one cell from water clips river bends.
                    let clearance = if self.config.prepared_routes {
                        1.36 * self.vehicle.size().max_element() / cell + 0.71
                    } else {
                        1.0
                    };
                    let terrain_near = (dx * dx + dy * dy) as f32 <= clearance * clearance;
                    let near = dx * dx + dy * dy <= 1;
                    let deliberate_breach = self.config.raiding
                        && self.config.siege_tactics
                        && self.vehicle != VehicleKind::Scout
                        && p.distance(goal) < 10.0;
                    let solid_near = if self.config.raiding && p.distance(goal) < 6.0 {
                        // Coarse clearance joins the four small flag posts into
                        // a sealed ring. Use their occupied cells for the final
                        // approach, with slow aligned steering below.
                        dx == 0 && dy == 0
                    } else {
                        terrain_near
                    };
                    costs[at] = costs[at].max(
                        if (self.config.safe_navigation
                            && v == 2
                            && terrain_near
                            && !deliberate_breach)
                            || (self.config.safe_navigation && v == 4 && solid_near)
                            || (v == 3 && terrain_near)
                            || (v == 4 && dx == 0 && dy == 0)
                            || (v == 2
                                && near
                                && self.config.vehicle_tactics
                                && self.vehicle == VehicleKind::Scout)
                        {
                            u32::MAX
                        } else if v >= 3 {
                            if near { 60 } else { 24 }
                        } else if near {
                            if deliberate_breach && self.breach.is_some() {
                                30
                            } else {
                                150
                            }
                        } else {
                            14
                        },
                    );
                }
            }
        }
        for &(point, _, radius) in &self.avoid {
            let at = self.map.index(point);
            if radius > 0 {
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let x = (at % w) as isize + dx;
                        let y = (at / w) as isize + dy;
                        if x >= 0 && y >= 0 && x < w as isize && y < h as isize {
                            costs[y as usize * w + x as usize] = u32::MAX;
                        }
                    }
                }
            } else {
                costs[at] = u32::MAX;
            }
        }
        if self.config.tactical_routes {
            for (i, cost) in costs
                .iter_mut()
                .enumerate()
                .filter(|(_, c)| **c != u32::MAX)
            {
                let p = self.map.point(i);
                // Remembered sightings, never hidden current positions. Route
                // around firing positions rather than repeatedly feeding the
                // same central choke. Near the flag, a breach still takes priority.
                for &(target, _) in &self.threats {
                    let radius = if target.mounted { 18.0 } else { 10.0 };
                    let distance = p.distance(target.position);
                    if distance < radius && p.distance(goal) > 10.0 {
                        *cost +=
                            ((1.0 - distance / radius) * self.config.threat_cost as f32) as u32;
                    }
                }
            }
        }
        if self.config.raiding
            && let Some(ally) = self.traffic_ally
        {
            for (i, cost) in costs
                .iter_mut()
                .enumerate()
                .filter(|(_, c)| **c != u32::MAX)
            {
                let distance = self.map.point(i).distance(ally.position);
                if distance < start.distance(ally.position).min(3.5) - 0.2 {
                    *cost = u32::MAX;
                    continue;
                }
                if distance < 6.0 {
                    *cost += ((1.0 - distance / 6.0) * 400.0) as u32;
                }
            }
        }
        if self.config.wide_routes
            && let Some(center) = self.battle_center()
        {
            for (i, cost) in costs
                .iter_mut()
                .enumerate()
                .filter(|(_, c)| **c != u32::MAX)
            {
                let distance = self.map.point(i).distance(center);
                if distance < 32.0 {
                    *cost += ((1.0 - distance / 32.0) * 160.0) as u32;
                }
            }
        }
        if self.config.prepared_routes {
            for (cost, prepared) in costs.iter_mut().zip(&self.prepared_costs) {
                if *cost != u32::MAX {
                    *cost = cost.saturating_add(*prepared);
                }
            }
        }
        let source = self.map.index(start);
        let destination = self.map.index(goal);
        if self.config.prepared_routes && costs[source] == u32::MAX && self.memory[source] < 2 {
            // A hull may already be inside the conservative bank margin after
            // contact. Let it leave that margin; a blocked source with no
            // neighbors otherwise falls back to driving straight at the flag.
            for (i, cost) in costs.iter_mut().enumerate() {
                let p = self.map.point(i);
                if *cost == u32::MAX
                    && self.memory[i] < 2
                    && p.distance(start) < 3.0
                    && p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(1.4)).all()
                    && !self
                        .avoid
                        .iter()
                        .any(|(a, _, r)| p.distance(*a) < (*r as f32 + 0.5) * cell)
                {
                    *cost = 1000;
                }
            }
        }
        costs[source] = 10;
        let heuristic = |i: usize| (self.map.point(i).distance(goal) / cell * 10.0) as u32;
        let mut distance = vec![u32::MAX; costs.len()];
        let mut previous = vec![usize::MAX; costs.len()];
        let mut queue = BinaryHeap::new();
        distance[source] = 0;
        queue.push(Reverse((heuristic(source), 0, source)));
        let mut closest = source;
        while let Some(Reverse((_, travelled, at))) = queue.pop() {
            if travelled != distance[at] {
                continue;
            }
            if heuristic(at) < heuristic(closest) {
                closest = at;
            }
            if at == destination {
                closest = at;
                break;
            }
            let x = (at % w) as isize;
            let y = (at / w) as isize;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let next = ny as usize * w + nx as usize;
                    if costs[next] == u32::MAX {
                        continue;
                    }
                    if dx != 0
                        && dy != 0
                        && (costs[y as usize * w + nx as usize] == u32::MAX
                            || costs[ny as usize * w + x as usize] == u32::MAX)
                    {
                        continue;
                    }
                    let d = travelled + costs[next] * if dx != 0 && dy != 0 { 14 } else { 10 } / 10;
                    if d < distance[next] {
                        distance[next] = d;
                        previous[next] = at;
                        queue.push(Reverse((d + heuristic(next), d, next)));
                    }
                }
            }
        }
        let mut route = vec![];
        let mut at = closest;
        while at != source {
            route.push(self.map.point(at));
            at = previous[at];
        }
        route.reverse();
        if self.config.raiding
            && closest == destination
            && (self.breach.is_some()
                || self.navigation_corridor(self.map.point(closest), goal, self.vehicle))
        {
            route.push(goal);
        }
        (route, distance[closest])
    }
}
pub fn polygon_distance(p: Vec2, poly: &[Vec2]) -> f32 {
    let mut positive = false;
    let mut negative = false;
    let mut distance = f32::INFINITY;
    for (a, b) in poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
    {
        let d = *b - *a;
        let cross = d.perp_dot(p - *a);
        positive |= cross > 0.0;
        negative |= cross < 0.0;
        let q = *a + d * ((p - *a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
        distance = distance.min(p.distance(q));
    }
    if positive && negative { distance } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn map() -> Map {
        Map {
            size: Vec2::splat(40.0),
            flags: vec![(Team::Red, Vec2::new(15.0, 0.0))],
            team: Team::Blue,
            layout: vec![1; 54 * 54],
            seed: 42,
            team_slot: 0,
            emplacements: vec![],
            supply_sites: vec![],
        }
    }
    pub(super) fn observation() -> Observation {
        Observation {
            tick: 0,
            position: Vec2::ZERO,
            hull_angle: 0.0,
            turret_angle: 0.0,
            alive: true,
            health: 100.0,
            reload: 0.0,
            aim_spread: 0.0,
            vehicle: VehicleKind::Tank,
            regions: vec![],
            special: None,
            supplies: vec![],
            allies: vec![],
            allied_vehicles: vec![],
            obstacles: vec![],
            targets: vec![],
            visible_ruins: vec![],
            terrain: vec![],
        }
    }
    #[test]
    fn new_sightings_cannot_trigger_fire_before_reaction_delay() {
        let mut controller = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            assert!(!controller.decide(&o).fire);
        }
        o.tick = 36;
        assert!(controller.decide(&o).fire);
    }
    #[test]
    fn deployment_stays_fixed_during_respawn_and_samples_all_vehicles() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        let mut kinds = vec![];
        for life in 0..30 {
            o.tick = life * 120;
            o.alive = false;
            let chosen = c.decide(&o).deploy.unwrap();
            kinds.push(chosen);
            o.tick += 12;
            assert_eq!(c.decide(&o).deploy, Some(chosen));
            o.tick += 12;
            o.alive = true;
            assert_eq!(c.decide(&o).deploy, None);
        }
        for kind in VehicleKind::ALL {
            assert!(kinds.contains(&kind));
        }
    }
    #[test]
    fn scout_turning_keeps_wheels_rolling() {
        let c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Scout;
        for heading in [1.0, 2.0, std::f32::consts::PI - 0.001] {
            let tracks = c.steer(&o, heading, 0.65);
            assert!(tracks[0] + tracks[1] > 0.1);
            assert!(tracks[0] > tracks[1]);
        }
    }
    #[test]
    fn committed_breacher_does_not_retreat_when_teammate_redeploys_as_a_tank() {
        let mut m = map();
        m.team_slot = 1;
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        o.allied_vehicles = vec![AlliedVehicle {
            position: Vec2::new(0.0, 8.0),
            vehicle: VehicleKind::Scout,
        }];
        for tick in [0, 12, 24, 36] {
            o.tick = tick;
            c.decide(&o);
        }
        assert_eq!(c.telemetry.role, "breacher");
        o.allied_vehicles[0].vehicle = VehicleKind::Tank;
        for tick in [48, 60, 72, 84] {
            o.tick = tick;
            c.decide(&o);
        }
        assert_eq!(c.known_ally.unwrap().0.vehicle, VehicleKind::Tank);
        assert_eq!(c.telemetry.role, "breacher");
        o.alive = false;
        o.tick = 96;
        c.decide(&o);
        o.alive = true;
        for tick in [108, 120, 132, 144] {
            o.tick = tick;
            c.decide(&o);
        }
        assert_eq!(c.telemetry.role, "support");
    }
    #[test]
    fn visible_ruins_are_delayed_before_retiring_public_defense_costs() {
        let mut m = map();
        m.emplacements.push(Emplacement {
            team: Team::Red,
            position: Vec2::new(10.0, 0.0),
            facing: 0.0,
            arc: 180.0,
            range: 7.0,
        });
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        o.visible_ruins.push(Vec2::new(10.0, 0.0));
        for tick in [0, 12, 24] {
            o.tick = tick;
            c.decide(&o);
            assert!(!c.retired_emplacements[0]);
        }
        o.tick = 36;
        c.decide(&o);
        assert!(c.retired_emplacements[0]);
    }
    #[test]
    fn surveying_an_enclosed_objective_still_engages_a_visible_fixed_defense() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(5.0, 0.0),
            mounted: true,
            vehicle: None,
        });
        o.obstacles.push(Obstacle {
            breakable: true,
            outline: vec![
                Vec2::new(14.0, -3.0),
                Vec2::new(16.0, -3.0),
                Vec2::new(16.0, 3.0),
                Vec2::new(14.0, 3.0),
            ],
        });
        let input = c.decide_perceived(&o);
        assert!(
            input.fire,
            "a surveying hold must not suppress the visible gun: {:?}",
            c.telemetry
        );
        assert_eq!(input.aim, Vec2::new(5.0, 0.0));
    }
    #[test]
    fn raiding_routes_reach_enclosed_public_objectives_without_crossing_water() {
        let level = crate::level::Level::from_json(include_str!("../../../levels/dota-level.json"))
            .unwrap();
        for team in [Team::Blue, Team::Red] {
            let world = crate::World::from_level(&level, team).unwrap();
            let m = world.ai_map();
            let flag = m.flags.iter().find(|(t, _)| *t == team.other()).unwrap().1;
            let mut goals = vec![flag];
            goals.extend(
                m.supply_sites
                    .iter()
                    .copied()
                    .filter(|p| p.x.abs() < 40.0 && p.y.abs() < 15.0),
            );
            let c = Controller::new(
                Config {
                    raiding: true,
                    ..Default::default()
                },
                m,
            );
            for goal in goals {
                let route = c.plan(world.launcher_position(), goal);
                assert!(
                    route.last().is_some_and(|p| p.distance(goal) < 1.5),
                    "{team:?} cannot route to {goal:?}: {:?}",
                    route.last()
                );
                assert!(route.iter().all(|p| c.memory[c.map.index(*p)] != 3));
                assert!(
                    route
                        .iter()
                        .all(|p| c.memory[c.map.index(*p)] != 2 || p.distance(goal) < 11.0)
                );
            }
        }
    }
    #[test]
    fn special_weapons_respect_inventory_aim_allies_and_cooldown() {
        for weapon in [
            SpecialWeapon::Rocket,
            SpecialWeapon::Laser,
            SpecialWeapon::Plasma,
            SpecialWeapon::Mine,
        ] {
            let mut o = observation();
            o.targets.push(Target {
                position: Vec2::new(12.0, 0.0),
                mounted: false,
                vehicle: None,
            });
            o.special = Some(SpecialState {
                weapon,
                charges: 1,
                cooldown: 0.0,
                active: false,
            });
            let mut c = Controller::new(Config::default(), map());
            assert!(c.decide_perceived(&o).special, "{weapon:?}");
            o.special.as_mut().unwrap().cooldown = 1.0;
            assert!(!c.decide_perceived(&o).special);
            o.special.as_mut().unwrap().cooldown = 0.0;
            o.vehicle = VehicleKind::Scout;
            assert!(!c.decide_perceived(&o).special);
            o.vehicle = VehicleKind::Tank;
            o.allies.push(Vec2::new(8.0, 0.0));
            if weapon != SpecialWeapon::Mine {
                assert!(!c.decide_perceived(&o).special);
            }
        }
    }
    #[test]
    fn visible_pickup_changes_route_only_after_reaction_delay() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.supplies.push(Supply {
            position: Vec2::new(5.0, 1.0),
            weapon: SpecialWeapon::Laser,
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            c.decide(&o);
            assert_eq!(c.telemetry.goal, [15.0, 0.0]);
        }
        o.tick = 36;
        c.decide(&o);
        assert_eq!(c.telemetry.goal, [5.0, 1.0]);
    }
    #[test]
    fn objective_scout_keeps_flag_route_when_an_enemy_is_visible() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Scout;
        o.targets.push(Target {
            position: Vec2::new(10.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        let input = c.decide_perceived(&o);
        assert_eq!(c.telemetry.goal, [15.0, 0.0]);
        assert!(input.tracks.iter().sum::<f32>() > 0.1);
        assert!(input.fire);
    }
    #[test]
    fn objective_route_ignores_long_pickup_detours() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.supplies.push(Supply {
            position: Vec2::new(-10.0, 0.0),
            weapon: SpecialWeapon::Laser,
        });
        c.decide_perceived(&o);
        assert_eq!(c.telemetry.goal, [15.0, 0.0]);
    }
    #[test]
    fn aiming_uses_only_motion_estimated_from_delayed_sightings() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        for tick in [0, 12, 24, 36] {
            o.tick = tick;
            o.targets[0].position.y = tick as f32 / 120.0 * 4.0;
            let input = c.decide(&o);
            if tick < 36 {
                assert!(!input.fire);
            } else {
                assert_eq!(input.aim.y, 0.0);
            }
        }
        o.tick = 48;
        o.targets[0].position.y = 1.6;
        let input = c.decide(&o);
        assert!(input.aim.y > 0.4 && input.aim.y < 1.6);
        o.tick = 120;
        o.targets.clear();
        c.decide(&o);
        o.tick = 168;
        let input = c.decide(&o);
        assert!(!input.fire);
        assert!(input.aim.y <= 1.6);
    }
    #[test]
    fn unsettled_reticle_prevents_wasting_a_shell() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: true,
            vehicle: None,
        });
        o.aim_spread = 30.0_f32.to_radians();
        assert!(!c.decide_perceived(&o).fire);
        o.aim_spread = 3.0_f32.to_radians();
        assert!(c.decide_perceived(&o).fire);
    }
    #[test]
    fn mortar_stops_and_aims_at_a_wall_outside_its_minimum_range() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Mortar;
        o.aim_spread = 1.0_f32.to_radians();
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(10.0, -20.0),
                Vec2::new(10.5, -20.0),
                Vec2::new(10.5, 20.0),
                Vec2::new(10.0, 20.0),
            ],
            breakable: true,
        });
        let input = c.decide_perceived(&o);
        assert!(input.aim.x > 9.0 && input.aim.x < 11.0, "{:?}", input.aim);
        assert!(input.tracks.iter().sum::<f32>().abs() < 0.001);
        assert!(input.tracks.iter().all(|v| v.abs() < 0.01));
        assert!(input.fire);
    }
    #[test]
    fn mortar_combat_hold_keeps_turning_toward_a_visible_enemy() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Mortar;
        o.targets.push(Target {
            position: Vec2::new(14.0, 8.0),
            mounted: true,
            vehicle: None,
        });
        let input = c.decide_perceived(&o);
        assert!(input.tracks[0] > 0.0 && input.tracks[1] < 0.0);
        assert!(!input.fire);
        assert_eq!(c.telemetry.mode, "mortar aiming");
    }
    #[test]
    fn mortar_can_fire_over_an_ally_but_not_onto_one() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Mortar;
        o.targets.push(Target {
            position: Vec2::new(14.0, 0.0),
            mounted: true,
            vehicle: None,
        });
        o.allies.push(Vec2::new(6.0, 0.0));
        assert!(c.decide_perceived(&o).fire);
        o.allies[0] = Vec2::new(13.0, 0.0);
        assert!(!c.decide_perceived(&o).fire);
    }
    #[test]
    fn tank_backs_out_when_visible_cover_blocks_its_barrel() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(1.2, -0.4),
                Vec2::new(1.4, -0.4),
                Vec2::new(1.4, 0.4),
                Vec2::new(1.2, 0.4),
            ],
            breakable: false,
        });
        let input = c.decide_perceived(&o);
        assert!(!input.fire);
        assert!(input.tracks.iter().sum::<f32>() < 0.0);
        assert_eq!(c.telemetry.mode, "clearing muzzle");
    }
    #[test]
    fn locomotion_recovery_does_not_interrupt_a_wall_firing_hold() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(5.9, -20.0),
                Vec2::new(6.2, -20.0),
                Vec2::new(6.2, 20.0),
                Vec2::new(5.9, 20.0),
            ],
            breakable: true,
        });
        for tick in (0..1320).step_by(12) {
            o.tick = tick;
            c.decide_perceived(&o);
            assert_eq!(c.telemetry.recoveries, 0, "at {tick}");
            assert_eq!(c.telemetry.mode, "clearing");
        }
        assert!(c.breach.is_some());
    }
    #[test]
    fn breach_must_fit_the_hull_before_the_bot_enters() {
        let c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(5.0, 0.8),
                Vec2::new(5.4, 0.8),
                Vec2::new(5.4, 4.0),
                Vec2::new(5.0, 4.0),
            ],
            breakable: true,
        });
        assert!(c.breach_obstruction(&o, Vec2::new(10.0, 0.0)).is_some());
        for p in &mut o.obstacles[0].outline {
            p.y += 1.0;
        }
        assert!(c.breach_obstruction(&o, Vec2::new(10.0, 0.0)).is_none());
    }
    #[test]
    fn supporting_mortar_does_not_compete_for_the_breachers_pickup() {
        let mut m = map();
        let pickup = Vec2::new(3.0, 6.0);
        m.supply_sites.push(pickup);
        m.team_slot = 1;
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        o.vehicle = VehicleKind::Mortar;
        o.allied_vehicles.push(AlliedVehicle {
            position: Vec2::new(5.0, 0.0),
            vehicle: VehicleKind::Tank,
        });
        o.supplies.push(Supply {
            position: pickup,
            weapon: SpecialWeapon::Rocket,
        });
        c.decide_perceived(&o);
        assert_eq!(c.telemetry.role, "support");
        assert_ne!(c.telemetry.goal, pickup.to_array());
        assert!(c.supply_goal.is_none());
    }
    #[test]
    fn raider_arms_at_a_nearby_public_site_before_crossing_the_map() {
        let mut m = map();
        m.size = Vec2::splat(160.0);
        m.layout = vec![1; 160 * 160];
        m.supply_sites = vec![Vec2::new(34.0, 7.0), Vec2::new(-34.0, 0.0)];
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        o.position = Vec2::new(43.0, -73.0);
        assert_eq!(
            c.armament_goal(&o, Vec2::new(-66.0, 64.0)),
            Some(Vec2::new(34.0, 7.0))
        );
    }
    #[test]
    fn tank_finishes_the_clear_approach_inside_a_flag_room() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.position = Vec2::new(12.0, 0.0);
        o.targets.push(Target {
            position: Vec2::new(6.0, 0.0),
            mounted: true,
            vehicle: None,
        });
        let input = c.decide_perceived(&o);
        assert_eq!(c.telemetry.mode, "capturing flag");
        assert_eq!(c.telemetry.waypoint, [15.0, 0.0]);
        assert!(input.tracks.iter().all(|v| *v > 0.0));
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(13.0, -2.0),
                Vec2::new(13.5, -2.0),
                Vec2::new(13.5, 2.0),
                Vec2::new(13.0, 2.0),
            ],
            breakable: false,
        });
        c.decide_perceived(&o);
        assert_ne!(c.telemetry.mode, "capturing flag");
    }
    #[test]
    fn armed_tank_retraces_its_entrance_out_of_a_narrow_supply_site() {
        let mut m = map();
        for i in 0..m.layout.len() {
            let p = m.point(i);
            if p.x.abs() < 3.0 && (p.y.abs() - 2.0).abs() < 0.5 {
                m.layout[i] = 2;
            }
        }
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        for (tick, x) in [(0, -12.0), (12, -8.0), (24, -4.0), (36, -1.5), (48, 0.0)] {
            o.tick = tick;
            o.position.x = x;
            c.decide(&o);
        }
        o.tick = 60;
        o.special = Some(SpecialState {
            weapon: SpecialWeapon::Rocket,
            charges: 3,
            cooldown: 0.0,
            active: false,
        });
        let input = c.decide(&o);
        assert_eq!(c.telemetry.mode, "leaving supply");
        assert!(
            input.tracks.iter().all(|v| *v < 0.0),
            "must back through the entrance: {:?}",
            input.tracks
        );
        assert!(c.telemetry.goal[0] < -8.0);
        o.alive = false;
        c.decide(&o);
        assert!(c.supply_exit.is_empty());
    }
    #[test]
    fn two_committed_breachers_have_asymmetric_traffic_priority() {
        let config = Config {
            raiding: true,
            ..Default::default()
        };
        let mut m = map();
        let mut first = Controller::new(config.clone(), m.clone());
        m.team_slot = 1;
        let mut second = Controller::new(config, m);
        let mut a = observation();
        a.allied_vehicles.push(AlliedVehicle {
            position: Vec2::new(3.0, 0.0),
            vehicle: VehicleKind::Tank,
        });
        let mut b = a.clone();
        b.position = Vec2::new(3.0, 0.0);
        b.hull_angle = std::f32::consts::PI;
        b.allied_vehicles[0].position = Vec2::ZERO;
        second.traffic_ally = b.allied_vehicles.first().copied();
        assert_eq!(first.team_traffic(&a, "breacher", [0.5; 2]), Some([0.0; 2]));
        assert_eq!(
            second.team_traffic(&b, "breacher", [0.5; 2]),
            Some([-0.25; 2])
        );
        // A mortar that was promoted after losing contact still gives a tank
        // priority; cached strategic roles cannot make both vehicles stop.
        b.vehicle = VehicleKind::Mortar;
        second.yield_goal = None;
        second.yield_reverse_until = 0;
        assert_ne!(
            second.team_traffic(&b, "breacher", [0.5; 2]),
            Some([0.0; 2])
        );
    }
    #[test]
    fn teammates_assign_complementary_roles_and_yield_right_of_way() {
        let mut m = map();
        m.team_slot = 0;
        let mut lead = Controller::new(Config::default(), m.clone());
        m.team_slot = 1;
        let mut support = Controller::new(Config::default(), m);
        let mut a = observation();
        a.allied_vehicles.push(AlliedVehicle {
            position: Vec2::new(2.5, 0.0),
            vehicle: VehicleKind::Tank,
        });
        let mut b = a.clone();
        b.position = Vec2::new(2.5, 0.0);
        b.allied_vehicles[0].position = Vec2::ZERO;
        let lead_input = lead.decide_perceived(&a);
        let support_input = support.decide_perceived(&b);
        assert_eq!(lead.telemetry.role, "breacher");
        assert_eq!(support.telemetry.role, "support");
        assert_ne!(lead.telemetry.goal, support.telemetry.goal);
        assert_eq!(lead_input.tracks, [0.0; 2]);
        assert_ne!(support_input.tracks, [0.0; 2]);
        a.vehicle = VehicleKind::Scout;
        assert_eq!(lead.team_role(&a), "support");
        b.allied_vehicles[0].vehicle = VehicleKind::Scout;
        assert_eq!(support.team_role(&b), "breacher");
    }
    #[test]
    fn allied_role_changes_wait_for_the_same_reaction_delay() {
        let mut m = map();
        m.team_slot = 1;
        let mut c = Controller::new(Config::default(), m);
        let mut o = observation();
        o.allied_vehicles.push(AlliedVehicle {
            position: Vec2::new(5.0, 0.0),
            vehicle: VehicleKind::Tank,
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            c.decide(&o);
            assert_eq!(c.telemetry.role, "independent");
        }
        o.tick = 36;
        c.decide(&o);
        assert_eq!(c.telemetry.role, "support");
    }
    #[test]
    fn committed_teammates_pass_head_on_using_physical_tracks() {
        teammates_pass_head_on([VehicleKind::Tank; 2]);
    }
    #[test]
    fn heavy_teammates_can_yield_and_pass_using_physical_tracks() {
        teammates_pass_head_on([VehicleKind::Tank, VehicleKind::Mortar]);
        teammates_pass_head_on([VehicleKind::Mortar, VehicleKind::Tank]);
        teammates_pass_head_on([VehicleKind::Mortar; 2]);
    }
    fn teammates_pass_head_on(kinds: [VehicleKind; 2]) {
        // Separate spawn markers give opposing initial headings. The two
        // controllers receive only the other visible hull as an ally, exactly
        // as two same-team vehicles would; no pose is changed after deployment.
        let mut level = crate::level::Level {
            size: [80.0, 60.0],
            ..Default::default()
        };
        for item in &mut level.items {
            match item {
                crate::level::LevelItem::Spawn {
                    team,
                    position,
                    angle,
                } => {
                    *position = if *team == Team::Blue {
                        [-12.0, 0.0]
                    } else {
                        [12.0, 0.0]
                    };
                    *angle = if *team == Team::Blue {
                        0.0
                    } else {
                        std::f32::consts::PI
                    };
                }
                crate::level::LevelItem::Flag { position, .. } => *position = [0.0, 25.0],
                _ => {}
            }
        }
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = std::array::from_fn::<_, 2, _>(|i| PlayerInput {
            deploy: Some(kinds[i]),
            ..Default::default()
        });
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bots: Vec<_> = (0..2)
            .map(|i| {
                let mut m = game.for_player(i).unwrap().ai_map();
                m.team = Team::Blue;
                m.team_slot = i;
                let mut c = Controller::new(
                    Config {
                        raiding: true,
                        ..Default::default()
                    },
                    m,
                );
                c.life_role = Some("breacher");
                c.set_patrol_goal(Vec2::new(if i == 0 { 25.0 } else { -25.0 }, 0.0));
                c
            })
            .collect();
        for tick in 0..if kinds.contains(&VehicleKind::Mortar) {
            3600
        } else {
            2400
        } {
            if tick % 12 == 0 {
                let observations: Vec<_> = (0..2)
                    .map(|i| game.for_player(i).unwrap().ai_observation())
                    .collect();
                for i in 0..2 {
                    let mut o = observations[i].clone();
                    let other = &observations[1 - i];
                    if o.targets
                        .iter()
                        .any(|t| t.position.distance(other.position) < 1.0)
                    {
                        o.allied_vehicles.push(AlliedVehicle {
                            position: other.position,
                            vehicle: other.vehicle,
                        });
                        o.allies.push(other.position);
                    }
                    o.targets.clear();
                    inputs[i] = bots[i].decide(&o);
                }
            }
            game.step(&inputs);
        }
        let a = game.for_player(0).unwrap().launcher_position();
        let b = game.for_player(1).unwrap().launcher_position();
        assert!(
            a.x > 10.0 && b.x < -10.0,
            "head-on {kinds:?} remain blocked: {a:?} / {b:?}"
        );
    }
    #[test]
    fn stationary_screen_does_not_block_a_breacher_following_a_river_waypoint() {
        let mut level = crate::level::Level {
            size: [80.0, 60.0],
            ..Default::default()
        };
        for item in &mut level.items {
            match item {
                crate::level::LevelItem::Spawn {
                    team,
                    position,
                    angle,
                } => {
                    *position = if *team == Team::Blue {
                        [0.0, -12.0]
                    } else {
                        [0.0, -3.0]
                    };
                    *angle = std::f32::consts::FRAC_PI_2;
                }
                crate::level::LevelItem::Flag { position, .. } => *position = [-25.0, 25.0],
                _ => {}
            }
        }
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
            PlayerInput {
                deploy: Some(VehicleKind::Scout),
                ..Default::default()
            },
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bots: Vec<_> = (0..2)
            .map(|i| {
                let mut m = game.for_player(i).unwrap().ai_map();
                m.team = Team::Blue;
                m.team_slot = i;
                let mut c = Controller::new(
                    Config {
                        raiding: true,
                        speed: 1.0,
                        ..Default::default()
                    },
                    m,
                );
                c.life_role = Some(if i == 0 { "breacher" } else { "support" });
                c
            })
            .collect();
        let goal = Vec2::new(0.0, 20.0);
        bots[0].set_patrol_goal(goal);
        for tick in 0..3000 {
            if tick % 12 == 0 {
                let observations: Vec<_> = (0..2)
                    .map(|i| game.for_player(i).unwrap().ai_observation())
                    .collect();
                for i in 0..2 {
                    let mut o = observations[i].clone();
                    let ally = &observations[1 - i];
                    if o.targets
                        .iter()
                        .any(|t| t.position.distance(ally.position) < 1.0)
                    {
                        o.allied_vehicles.push(AlliedVehicle {
                            position: ally.position,
                            vehicle: ally.vehicle,
                        });
                        o.allies.push(ally.position);
                    }
                    o.targets.clear();
                    inputs[i] = bots[i].decide(&o);
                    inputs[i].fire = false;
                    inputs[i].special = false;
                }
            }
            game.step(&inputs);
            if game
                .for_player(0)
                .unwrap()
                .launcher_position()
                .distance(goal)
                < 3.0
            {
                for player in 0..2 {
                    assert_eq!(
                        game.for_player(player).unwrap().ai_observation().health,
                        100.0,
                        "passing caused collision damage"
                    );
                }
                return;
            }
        }
        panic!(
            "screen blocked the breacher at {:?}",
            game.for_player(0).unwrap().launcher_position()
        );
    }
    #[test]
    fn perimeter_missions_choose_opposite_safe_flanks_and_do_not_order_a_retreat() {
        let level = crate::level::Level::from_json(include_str!("../../../levels/dota-level.json"))
            .unwrap();
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        for side in 0..2 {
            let world = game.for_player(side).unwrap();
            let m = world.ai_map();
            let mut o = world.ai_observation();
            let objective = m.flags.iter().find(|(t, _)| *t != m.team).unwrap().1;
            let mut c = Controller::new(
                Config {
                    raiding: true,
                    perimeter_routes: true,
                    ..Default::default()
                },
                m,
            );
            let goal = c.flanking_goal(&o, objective).unwrap();
            assert!(
                if side == 0 {
                    goal.x < -30.0 && goal.y < -30.0
                } else {
                    goal.x > 30.0 && goal.y > 30.0
                },
                "wrong flank: {goal:?}"
            );
            assert_eq!(c.prepared_costs[c.map.index(goal)], 0);
            o.position = if side == 0 {
                Vec2::new(25.0, -8.0)
            } else {
                Vec2::new(-4.0, 5.0)
            };
            assert_eq!(
                c.flanking_goal(&o, objective),
                Some(goal),
                "a river crossing is not completion of the perimeter leg"
            );
            o.position = objective + Vec2::X * 20.0;
            assert!(c.flanking_goal(&o, objective).is_none());
        }
    }
    #[test]
    fn support_holds_the_front_when_a_visible_leader_has_redeployed() {
        let mut m = map();
        m.size = Vec2::splat(160.0);
        m.layout = vec![1; 160 * 160];
        m.flags = vec![(Team::Red, Vec2::new(70.0, 0.0))];
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        c.life_role = Some("support");
        let mut o = observation();
        o.position = Vec2::new(40.0, 0.0);
        o.vehicle = VehicleKind::Scout;
        o.allied_vehicles.push(AlliedVehicle {
            position: Vec2::new(-50.0, 0.0),
            vehicle: VehicleKind::Tank,
        });
        c.remember_teammate(&o);
        let goal = c.team_goal(&o, "support", Vec2::new(70.0, 0.0)).unwrap();
        assert!(goal.x > 30.0, "escort was ordered home: {goal:?}");
        o.vehicle = VehicleKind::Mortar;
        c.remember_teammate(&o);
        assert_eq!(c.team_role(&o), "breacher");
        assert!(c.support_goal.is_none());
    }
    #[test]
    fn mobile_engagement_keeps_the_tank_moving_and_preserves_point_blank_escape() {
        let config = Config {
            raiding: true,
            proactive_combat: true,
            mobile_combat: true,
            combat_range: 18.0,
            ..Default::default()
        };
        let mut c = Controller::new(config, map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: Some(VehicleKind::Tank),
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            c.decide(&o);
        }
        o.tick = 36;
        let input = c.decide(&o);
        assert_eq!(c.telemetry.mode, "mobile combat");
        assert!(input.tracks.iter().sum::<f32>() > 0.1);
        assert!(input.fire);
        o.targets[0].position = Vec2::new(2.5, 0.0);
        let input = c.decide_perceived(&o);
        assert_ne!(c.telemetry.mode, "mobile combat");
        assert!(input.tracks.iter().sum::<f32>() < 0.0);
    }
    #[test]
    fn newly_recognized_enemy_chassis_has_the_same_reaction_delay() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                proactive_combat: true,
                combat_range: 18.0,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        for tick in [0, 12, 24, 36] {
            o.tick = tick;
            c.decide(&o);
        }
        o.targets[0].vehicle = Some(VehicleKind::Mortar);
        for tick in [48, 60, 72] {
            o.tick = tick;
            c.decide(&o);
            assert_ne!(c.telemetry.mode, "flanking mortar");
        }
        o.tick = 84;
        c.decide(&o);
        assert_eq!(c.telemetry.mode, "flanking mortar");
    }
    #[test]
    fn tank_keeps_moving_when_engaging_a_visible_mortar() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                proactive_combat: true,
                combat_range: 18.0,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: Some(VehicleKind::Mortar),
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            c.decide(&o);
        }
        o.tick = 36;
        let input = c.decide(&o);
        assert_eq!(c.telemetry.mode, "flanking mortar");
        assert_ne!(input.tracks, [0.0; 2]);
        assert!(input.fire);
    }
    #[test]
    fn proactive_combat_waits_for_delayed_sight_even_with_an_undamaged_tank() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                proactive_combat: true,
                combat_range: 18.0,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        for tick in [0, 12, 24] {
            o.tick = tick;
            assert!(!c.decide(&o).fire);
        }
        o.tick = 36;
        let input = c.decide(&o);
        assert_eq!(c.telemetry.mode, "engaging vehicle");
        assert!(input.fire);
        assert_eq!(input.tracks, [0.0; 2]);
    }
    #[test]
    fn damaged_breacher_returns_fire_then_resumes_its_wall_breach() {
        let mut c = Controller::new(
            Config {
                raiding: true,
                combat_range: 18.0,
                ..Default::default()
            },
            map(),
        );
        let mut o = observation();
        o.position = Vec2::new(5.0, 0.0);
        o.health = 88.0;
        o.turret_angle = std::f32::consts::FRAC_PI_2;
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(10.0, -8.0),
                Vec2::new(10.5, -8.0),
                Vec2::new(10.5, 8.0),
                Vec2::new(10.0, 8.0),
            ],
            breakable: true,
        });
        o.targets.push(Target {
            position: Vec2::new(5.0, 8.0),
            mounted: false,
            vehicle: None,
        });
        c.breach = Some(Breach {
            objective: Vec2::new(15.0, 0.0),
            exit: Vec2::new(15.0, 0.0),
            since: 0,
            shots: 0,
            last_reload: 0.0,
            drive_until: 0,
        });
        let input = c.decide_perceived(&o);
        assert_eq!(c.telemetry.mode, "returning fire");
        assert_eq!(input.aim, Vec2::new(5.0, 8.0));
        assert!(input.fire);
        assert!(c.breach.is_some());
        o.targets.clear();
        o.tick = 120;
        let input = c.decide_perceived(&o);
        assert_ne!(c.telemetry.mode, "returning fire");
        assert!(
            input.aim.x > 9.0,
            "must resume clearing the wall: {:?}",
            input.aim
        );
    }
    #[test]
    fn base_staging_does_not_choose_the_small_rock_enclosure_south_of_red_flag() {
        let level = crate::level::Level::from_json(include_str!("../../../levels/dota-level.json"))
            .unwrap();
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        let m = game.for_player(0).unwrap().ai_map();
        let flag = m
            .flags
            .iter()
            .find(|(team, _)| *team == Team::Red)
            .unwrap()
            .1;
        let mut c = Controller::new(
            Config {
                raiding: true,
                ..Default::default()
            },
            m,
        );
        let mut o = observation();
        o.position = Vec2::new(43.35, -73.33);
        let goal = c.assault_goal(&o, flag, flag, "breacher");
        assert!(c.assault_approach.is_some());
        assert!(
            goal.distance(flag - Vec2::Y * 14.0) > 2.0,
            "staging entered the rock enclosure: {goal:?}"
        );
    }
    #[test]
    fn scout_screen_does_not_drag_the_breacher_back_and_forth_beside_the_river() {
        let mut level =
            crate::level::Level::from_json(include_str!("../../../levels/dota-level.json"))
                .unwrap();
        level.items.retain(|i| {
            !matches!(
                i,
                crate::level::LevelItem::Turret { .. } | crate::level::LevelItem::Pickup { .. }
            )
        });
        for item in &mut level.items {
            if let crate::level::LevelItem::Spawn {
                team,
                position,
                angle,
            } = item
            {
                *position = if *team == Team::Blue {
                    [-65.0, 28.0]
                } else {
                    [-73.0, 36.0]
                };
                *angle = std::f32::consts::FRAC_PI_2;
            }
        }
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [
            PlayerInput {
                deploy: Some(VehicleKind::Tank),
                ..Default::default()
            },
            PlayerInput {
                deploy: Some(VehicleKind::Scout),
                ..Default::default()
            },
        ];
        for _ in 0..3 {
            game.step(&inputs);
        }
        let mut bots: Vec<_> = (0..2)
            .map(|i| {
                let mut m = game.for_player(i).unwrap().ai_map();
                m.team = Team::Blue;
                m.team_slot = i;
                let mut c = Controller::new(
                    Config {
                        raiding: true,
                        speed: 1.0,
                        rolling_tracks: false,
                        ..Default::default()
                    },
                    m,
                );
                c.life_role = Some(if i == 0 { "breacher" } else { "support" });
                c
            })
            .collect();
        let goal = Vec2::new(-65.0, 43.0);
        bots[0].set_patrol_goal(goal);
        for tick in 0..5400 {
            if tick % 12 == 0 {
                let observations: Vec<_> = (0..2)
                    .map(|i| game.for_player(i).unwrap().ai_observation())
                    .collect();
                for i in 0..2 {
                    let mut o = observations[i].clone();
                    let ally = &observations[1 - i];
                    if o.targets
                        .iter()
                        .any(|t| t.position.distance(ally.position) < 1.0)
                    {
                        o.allied_vehicles.push(AlliedVehicle {
                            position: ally.position,
                            vehicle: ally.vehicle,
                        });
                        o.allies.push(ally.position);
                    }
                    o.targets.clear();
                    inputs[i] = bots[i].decide(&o);
                    inputs[i].fire = false;
                    inputs[i].special = false;
                }
            }
            game.step(&inputs);
            if game
                .for_player(0)
                .unwrap()
                .launcher_position()
                .distance(goal)
                < 3.0
            {
                return;
            }
        }
        panic!(
            "escort trapped the breacher at {:?}",
            game.for_player(0).unwrap().launcher_position()
        );
    }
    #[test]
    fn two_tanks_leave_spawn_without_blocking_each_other() {
        let mut level = crate::level::Level {
            size: [80.0, 60.0],
            ..Default::default()
        };
        for item in &mut level.items {
            match item {
                crate::level::LevelItem::Spawn { team, position, .. } => {
                    *position = if *team == Team::Blue {
                        [-30.0, 0.0]
                    } else {
                        [30.0, 0.0]
                    };
                }
                crate::level::LevelItem::Flag { team, position } => {
                    *position = if *team == Team::Blue {
                        [-35.0, 0.0]
                    } else {
                        [35.0, 0.0]
                    };
                }
                _ => {}
            }
        }
        let mut game = crate::multiplayer::MultiplayerWorld::new(&level, &ARENA_TEAMS).unwrap();
        game.require_vehicle_choices();
        let mut input = [PlayerInput {
            deploy: Some(VehicleKind::Tank),
            ..Default::default()
        }; 4];
        for _ in 0..3 {
            game.step(&input);
        }
        let mut bots: Vec<_> = [0, 2]
            .into_iter()
            .map(|i| {
                let mut m = game.for_player(i).unwrap().ai_map();
                m.team_slot = i / 2;
                (i, Controller::new(Config::default(), m))
            })
            .collect();
        for tick in 0..960 {
            if tick % 12 == 0 {
                for (i, c) in &mut bots {
                    let o = game.for_player(*i).unwrap().ai_observation();
                    input[*i] = c.decide(&o);
                }
            }
            game.step(&input);
        }
        let lead = game.for_player(0).unwrap().launcher_position();
        let support = game.for_player(2).unwrap().launcher_position();
        assert!(lead.x > -20.0, "lead stalled at {lead:?}");
        assert!(
            lead.distance(support) > 4.0,
            "vehicles still obstruct each other: {lead:?} / {support:?}"
        );
        assert_eq!(bots[0].1.telemetry.role, "breacher");
        assert_eq!(bots[1].1.telemetry.role, "support");
    }
    #[test]
    fn public_layout_routes_to_the_only_river_crossing() {
        let mut m = map();
        m.layout = vec![1; m.width() * m.height()];
        for i in 0..m.layout.len() {
            let p = m.point(i);
            if p.x.abs() < 2.0 && (p.y - 10.0).abs() > 3.0 {
                m.layout[i] = 3;
            }
        }
        let c = Controller::new(Config::default(), m);
        let route = c.plan(Vec2::new(-15.0, 0.0), Vec2::new(15.0, 0.0));
        assert!(route.last().unwrap().distance(Vec2::new(15.0, 0.0)) < 1.0);
        assert!(route.iter().any(|p| p.x.abs() < 2.0 && p.y > 7.0));
        assert!(route.iter().all(|p| c.memory[c.map.index(*p)] != 3));
        assert!(!c.corridor_clear(Vec2::new(-5.0, 0.0), Vec2::new(5.0, 0.0), VehicleKind::Tank));
    }
    #[test]
    fn tank_drives_around_a_river_bend_with_its_whole_hull() {
        use crate::level::LevelItem;
        let level = crate::level::Level {
            size: [60.0, 60.0],
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [15.0, -15.0],
                    angle: 2.4,
                },
                LevelItem::Spawn {
                    team: Team::Red,
                    position: [-25.0, 25.0],
                    angle: 0.0,
                },
                LevelItem::Flag {
                    team: Team::Blue,
                    position: [25.0, -25.0],
                },
                LevelItem::Flag {
                    team: Team::Red,
                    position: [-15.0, 15.0],
                },
                LevelItem::Terrain {
                    terrain: crate::TerrainKind::Water,
                    min: [-5.0, -25.0],
                    max: [5.0, 5.0],
                },
            ],
            ..Default::default()
        };
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        let mut c = Controller::new(Config::default(), game.for_player(0).unwrap().ai_map());
        let mut input = [PlayerInput::default(); 2];
        for tick in 0..3600 {
            if tick % 12 == 0 {
                input[0] = c.decide(&game.for_player(0).unwrap().ai_observation());
            }
            game.step(&input);
            if game.world().winner() == Some(Team::Blue) {
                return;
            }
        }
        panic!(
            "river approach stalled at {:?}: {:?}",
            game.for_player(0).unwrap().launcher_position(),
            c.telemetry
        );
    }
    #[test]
    fn route_flanks_the_public_starting_defense_arc() {
        let mut m = map();
        m.emplacements.push(Emplacement {
            team: Team::Red,
            position: Vec2::ZERO,
            facing: 0.0,
            arc: 180.0,
            range: 7.0,
        });
        let c = Controller::new(Config::default(), m);
        let route = c.plan(Vec2::new(-15.0, 0.0), Vec2::new(15.0, 0.0));
        assert!(route.last().unwrap().distance(Vec2::new(15.0, 0.0)) < 1.0);
        assert!(route.iter().all(|p| p.length() >= 9.0 || p.x < 0.0));
    }
    #[test]
    fn river_valley_public_routes_reach_both_base_approaches_without_crossing_scenery() {
        let level = crate::level::Level::from_json(include_str!("../../../levels/dota-level.json"))
            .unwrap();
        for team in [Team::Blue, Team::Red] {
            let world = crate::World::from_level(&level, team).unwrap();
            let map = world.ai_map();
            let goal = map
                .flags
                .iter()
                .find(|(t, _)| *t == team.other())
                .unwrap()
                .1;
            let c = Controller::new(Config::default(), map);
            let route = c.plan(world.launcher_position(), goal);
            assert!(
                route.last().is_some_and(|p| p.distance(goal) < 9.0),
                "{team:?} route stops at {:?}",
                route.last()
            );
            assert!(route.iter().all(|p| c.memory[c.map.index(*p)] < 2));
        }
    }
    #[test]
    fn acquired_special_has_its_own_reaction_delay() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        for tick in [0, 12, 24, 36] {
            o.tick = tick;
            c.decide(&o);
        }
        o.special = Some(SpecialState {
            weapon: SpecialWeapon::Laser,
            charges: 1,
            cooldown: 0.0,
            active: false,
        });
        for tick in [48, 60, 72] {
            o.tick = tick;
            assert!(!c.decide(&o).special);
        }
        o.tick = 84;
        assert!(c.decide(&o).special);
    }
    #[test]
    fn prolonged_clearing_blocks_an_area_for_a_new_route() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Scout;
        for tick in [0, 120, 360, 708] {
            o.tick = tick;
            assert!(!c.watch_clearing(&o, Some(Vec2::new(4.0, 0.0))));
        }
        o.tick = 720;
        assert!(c.watch_clearing(&o, Some(Vec2::new(4.0, 0.0))));
        assert_eq!(c.avoid.len(), 1);
        assert_eq!(c.avoid[0].1, 4320);
    }
    #[test]
    fn scout_recovery_never_requests_a_stationary_pivot() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Scout;
        let i = c.map.index(Vec2::new(-1.5, 0.0));
        c.memory[i] = 3;
        c.reverse_until = 100;
        let input = c.decide_perceived(&o);
        assert!(input.tracks.iter().sum::<f32>().abs() > 0.05);
    }
    #[test]
    fn scout_never_fires_its_vehicle_only_gun_at_cover() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.vehicle = VehicleKind::Scout;
        o.obstacles.push(Obstacle {
            outline: vec![
                Vec2::new(4.0, -3.0),
                Vec2::new(4.5, -3.0),
                Vec2::new(4.5, 3.0),
                Vec2::new(4.0, 3.0),
            ],
            breakable: true,
        });
        for tick in (0..1200).step_by(12) {
            o.tick = tick;
            assert!(!c.decide_perceived(&o).fire);
        }
    }
    #[test]
    fn scout_routes_around_breakable_walls_that_a_tank_can_clear() {
        let mut m = map();
        m.layout = vec![1; m.width() * m.height()];
        for i in 0..m.layout.len() {
            let p = m.point(i);
            if p.x.abs() < 1.0 && (p.y - 10.0).abs() > 3.0 {
                m.layout[i] = 2;
            }
        }
        let mut c = Controller::new(Config::default(), m);
        c.vehicle = VehicleKind::Scout;
        let route = c.plan(Vec2::new(-15.0, 0.0), Vec2::new(15.0, 0.0));
        assert!(route.last().unwrap().distance(Vec2::new(15.0, 0.0)) < 1.0);
        assert!(route.iter().all(|p| c.memory[c.map.index(*p)] < 2));
    }
    #[test]
    fn all_vehicle_types_reverse_out_of_an_unshootable_close_range() {
        for kind in VehicleKind::ALL {
            let mut c = Controller::new(Config::default(), map());
            let mut o = observation();
            o.vehicle = kind;
            o.targets.push(Target {
                position: Vec2::new(2.5, 0.0),
                mounted: false,
                vehicle: None,
            });
            let input = c.decide_perceived(&o);
            assert!(
                input.tracks.iter().sum::<f32>() < -0.1,
                "{kind:?}: {:?}",
                input.tracks
            );
            assert_eq!(c.telemetry.mode, "opening range");
            assert!(!input.fire);
        }
    }
    #[test]
    fn failed_cover_attack_is_abandoned_but_visible_progress_keeps_it_active() {
        for progress in [false, true] {
            let mut c = Controller::new(Config::default(), map());
            let mut o = observation();
            let point = Vec2::new(4.0, 0.0);
            let mut abandoned = false;
            for tick in (0..2400).step_by(12) {
                o.tick = tick;
                o.reload = if tick % 120 == 12 { 0.8 } else { 0.0 };
                let bottom = if progress {
                    -1.5 + (tick / 360) as f32 * 0.2
                } else {
                    -1.5
                };
                o.obstacles = vec![Obstacle {
                    outline: vec![
                        Vec2::new(3.8, bottom),
                        Vec2::new(4.4, bottom),
                        Vec2::new(4.4, 2.0),
                        Vec2::new(3.8, 2.0),
                    ],
                    breakable: true,
                }];
                abandoned |= c.watch_cover_progress(&o, Some(point));
            }
            assert_eq!(abandoned, !progress);
            assert_eq!(c.avoid.is_empty(), progress);
        }
    }
    #[test]
    fn close_tanks_separate_and_resume_actual_firing() {
        let mut level = crate::level::Level::default();
        for item in &mut level.items {
            if let crate::level::LevelItem::Spawn {
                team,
                position,
                angle,
            } = item
            {
                *position = if *team == Team::Blue {
                    [-1.3, 0.0]
                } else {
                    [1.3, 0.0]
                };
                *angle = if *team == Team::Blue {
                    0.0
                } else {
                    std::f32::consts::PI
                };
            }
        }
        let mut game =
            crate::multiplayer::MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        game.configure_airdrops(false, 42);
        let mut bots: Vec<_> = (0..2)
            .map(|i| Controller::new(Config::default(), game.for_player(i).unwrap().ai_map()))
            .collect();
        let mut inputs = [PlayerInput::default(); 2];
        let mut furthest = 0.0_f32;
        for tick in 0..1440 {
            if tick % 12 == 0 {
                for i in 0..2 {
                    inputs[i] = bots[i].decide(&game.for_player(i).unwrap().ai_observation());
                }
            }
            game.step(&inputs);
            let a = game.for_player(0).unwrap().launcher_position();
            let b = game.for_player(1).unwrap().launcher_position();
            furthest = furthest.max(a.distance(b));
        }
        assert!(furthest > 5.5, "distance only {furthest}");
        assert!(
            game.shots(0) > 0 && game.shots(1) > 0,
            "shots {} / {}",
            game.shots(0),
            game.shots(1)
        );
    }
    #[test]
    fn arena_roster_deploys_two_independently_controlled_vehicles_per_team() {
        let mut game = crate::multiplayer::MultiplayerWorld::new(
            &crate::level::Level::default(),
            &ARENA_TEAMS,
        )
        .unwrap();
        game.require_vehicle_choices();
        let mut bots: Vec<_> = (0..ARENA_PLAYERS)
            .map(|i| {
                let mut m = game.for_player(i).unwrap().ai_map();
                m.seed = player_seed(42, i);
                Controller::new(Config::default(), m)
            })
            .collect();
        let mut input = [PlayerInput::default(); ARENA_PLAYERS];
        let mut chosen = [None; ARENA_PLAYERS];
        for tick in 0..360 {
            if tick % 12 == 0 {
                for i in 0..ARENA_PLAYERS {
                    input[i] = bots[i].decide(&game.for_player(i).unwrap().ai_observation());
                    chosen[i] = chosen[i].or(input[i].deploy);
                }
            }
            game.step(&input);
            if chosen.iter().all(Option::is_some)
                && (0..ARENA_PLAYERS).all(|i| game.for_player(i).unwrap().tank_alive())
            {
                break;
            }
        }
        assert_eq!(game.player_count(), 4);
        for (i, kind) in chosen.iter().enumerate() {
            let w = game.for_player(i).unwrap();
            assert!(w.tank_alive());
            assert_eq!(Some(w.player_vehicle()), *kind);
        }
        assert_ne!(player_seed(42, 0), player_seed(42, 2));
        assert_eq!(ARENA_TEAMS.iter().filter(|&&t| t == Team::Blue).count(), 2);
        assert_eq!(ARENA_TEAMS.iter().filter(|&&t| t == Team::Red).count(), 2);
    }
    #[test]
    fn bots_move_out_of_a_teammates_firing_lane() {
        let mut c = Controller::new(Config::default(), map());
        let mut o = observation();
        o.targets.push(Target {
            position: Vec2::new(12.0, 0.0),
            mounted: false,
            vehicle: None,
        });
        o.allies.push(Vec2::new(6.0, 0.0));
        let input = c.decide_perceived(&o);
        assert!(!input.fire);
        assert_eq!(c.telemetry.mode, "clearing ally lane");
        assert_ne!(input.tracks, [0.0; 2]);
    }
}
