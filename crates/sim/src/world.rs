//! Owns simulation state and the fixed-step update order.
//! Public data lives in `api`; construction, queries and mechanics live in child modules.
use rapier2d::prelude::*;

use crate::{Ammunition, Material};
mod profiling;
#[cfg(feature = "physics-profile")]
pub use profiling::PhysicsProfile;
use profiling::profile;

pub const TICK_DT: f32 = 1.0 / 120.0;
const SUBSTEPS: usize = 4;
const DEPTH: f32 = 0.25;
mod aiming;
mod airdrops;
pub mod multiplayer;
pub mod replica;
mod specials;
pub use airdrops::AirdropView;
pub use specials::PickupView;
mod api;
pub use aiming::AimView;
mod debris;
mod queries;
mod stations;
mod tank_wreck;
mod vision;
pub use stations::StationView;
pub use vision::{NEAR_VISION, VISION_HALF_ANGLE, VISION_RANGE, Vision};
mod scenes;
mod setup;
pub use api::{
    BodyView, BondView, Command, DestructionCause, Event, LaserBeam, Scene, Shape, Shell, Shot,
    Stats, TankDestruction, TankFragment, TankImpact,
};

mod collision;
use collision::CollisionLayer;
mod bending;
mod chunks;
mod combat;
mod components;
mod contacts;
mod decorations;
mod enemy;
mod geometry;
mod level;
mod loads;
mod plasticity;
mod refinement;
mod terrain;
mod vehicle;
pub use combat::TankView;
mod weapons;

const DEBRIS_AREA: f32 = 0.075;
// Ground spin deceleration in rad/s² at friction 0.32, four times the old drag.
const FLOOR_ANGULAR_DECELERATION: f32 = 10.0;

#[derive(Clone, Copy)]
struct Solid {
    density: f32,
    depth: f32,
    strength: crate::material::Strength,
    young: f32,
    /// Illustrative fracture-surface work in J/m², not accumulated damage.
    toughness: f32,
    cohesion: f32,
}
impl Solid {
    fn new(material: Material) -> Self {
        let (young, toughness) = match material {
            Material::Rock => (40e9, 60.0),
            Material::Panel => (8e9, 120.0),
            _ => (15e9, 30.0),
        };
        Self {
            density: material.density(),
            depth: DEPTH,
            strength: material.strength(),
            young,
            toughness,
            cohesion: 1.0,
        }
    }
}

#[derive(Clone)]
struct Piece {
    reference_area: f32,
    /// Authored triangle edge lengths, retained as the deformable frame bends.
    reference_edges: [f32; 3],
    plastic_strain: f32,
    shape_revision: u32,
    body: RigidBodyHandle,
    shape: Shape,
    material: Material,
    previous_position: Vector,
    previous_angle: f32,
    load: f32,
    debris_remaining: Option<f32>,
    tank_fragment: Option<TankFragment>,
    floor_friction: f32,
    restitution: f32,
    soft_bounce: Option<crate::SoftBounce>,
    /// Ground attachment in this cell's local coordinates, inherited by only
    /// the refined child that contains it.
    ground_anchor: Option<Vector>,
    /// A bridge deck occupies the lower collision layer, below driving objects.
    bridge_deck: bool,
    collider: ColliderHandle,
    local_pose: Pose,
    active: bool,
    solid: Solid,
    refinement: u8,
    refinement_sources: Vec<u64>,
    origin: usize,
    fracture_parent: Option<usize>,
    grain: Option<usize>,
    object: Option<usize>,
    edges: u8,
}

impl Shape {
    fn area(self) -> f32 {
        match self {
            Self::Triangle {
                vertices: [a, b, c],
            } => (b - a).perp_dot(c - a).abs() * 0.5,
            Self::Box { half } => 4.0 * half.x * half.y,
            Self::Ball { radius } => std::f32::consts::PI * radius * radius,
        }
    }
    fn inertia(self, mass: f32) -> f32 {
        match self {
            Self::Triangle { vertices } => {
                mass * vertices.iter().map(|v| v.length_squared()).sum::<f32>() / 12.0
            }
            Self::Box { half } => mass * half.length_squared() / 3.0,
            Self::Ball { radius } => mass * radius * radius * 0.5,
        }
    }
}
impl Piece {
    fn new(
        id: usize,
        body: RigidBodyHandle,
        collider: ColliderHandle,
        shape: Shape,
        material: Material,
        position: Vector,
    ) -> Self {
        Self {
            reference_area: shape.area(),
            reference_edges: match shape {
                Shape::Triangle { vertices } => {
                    std::array::from_fn(|i| vertices[i].distance(vertices[(i + 1) % 3]))
                }
                _ => [0.0; 3],
            },
            plastic_strain: 0.0,
            shape_revision: 0,
            body,
            collider,
            shape,
            material,
            previous_position: position,
            previous_angle: 0.0,
            load: 0.0,
            debris_remaining: None,
            tank_fragment: None,
            floor_friction: 0.32,
            restitution: 0.04,
            soft_bounce: None,
            ground_anchor: None,
            bridge_deck: false,
            local_pose: Pose::IDENTITY,
            active: true,
            solid: Solid::new(material),
            refinement: 0,
            refinement_sources: Vec::new(),
            origin: id,
            fracture_parent: None,
            grain: None,
            object: None,
            edges: 7,
        }
    }
    fn destructible(&self) -> bool {
        matches!(
            self.material,
            Material::Masonry | Material::Panel | Material::Rock
        )
    }
    fn mass(&self) -> f32 {
        self.reference_area * self.solid.density * self.solid.depth
    }
}

#[derive(Clone)]
struct Bond {
    active: bool,
    a: usize,
    b: usize,
    anchor_a: Vector,
    normal_a: Vector,
    width: f32,
    strength: crate::material::Strength,
    depth: f32,
    load: f32,
    strength_scale: f32,
    young: f32,
    toughness: f32,
}

pub struct World {
    #[cfg(feature = "physics-profile")]
    profile: PhysicsProfile,
    replica: Option<Box<replica::Frame>>,
    sight_cache: Option<Vec<vision::Occluder>>,
    grass_cache: std::sync::Mutex<vision::GrassCache>,
    plastic_work: f64,
    physics: PhysicsWorld,
    pieces: Vec<Piece>,
    bonds: Vec<Bond>,
    // Bond endpoints and IDs are stable; refinement appends replacement bonds.
    incident_bonds: Vec<Vec<usize>>,
    ground_members: Vec<usize>,
    ground_records: usize,
    ground_members_dirty: bool,
    bending_joints: Vec<ImpulseJointHandle>,
    launcher: usize,
    tanks: Vec<combat::Tank>,
    enemy: Option<enemy::Enemy>,
    arena_size: Vector,
    scene: Scene,
    tick: u64,
    shots: usize,
    peak_load: f32,
    shells: Vec<Shell>,
    blast_pulses: Vec<weapons::BlastPulse>,
    explosions: usize,
    cleared: usize,
    broken: usize,
    topology_changed: bool,
    reload: f32,
    player_aim: aiming::AimState,
    primary_ammunition: Ammunition,
    special: Option<crate::SpecialWeapon>,
    special_charges: u32,
    special_interval: f32,
    special_reload: f32,
    pickups: Vec<specials::Pickup>,
    airdrops: airdrops::Airdrops,
    decorations: Vec<decorations::Decoration>,
    laser_remaining: f32,
    plasma_remaining: f32,
    plasma_direction: Vector,
    plasma_beam: Option<LaserBeam>,
    laser_pulse_in: f32,
    last_ammunition: Ammunition,
    laser_beam: Option<LaserBeam>,
    ablated: usize,
    ablated_area: f32,
    track_travel: [f32; 2],
    loads: Vec<loads::Load>,
    structural_cache: std::collections::BTreeMap<(u32, u32), crate::structure::System>,
    impact_serial: u64,
    refinements: usize,
    refinement_limited: usize,
    fracture_work: f64,
    structural_residual: f64,
    elastic_energy: f64,
    structural_iterations: usize,
    unconverged: usize,
    tank_speed_limit: f32,
    objects: Vec<components::ObjectInstance>,
    terrain: Vec<crate::TerrainPatch>,
    terrain_mask: Option<crate::level::TerrainMask>,
    bridges: Vec<terrain::Bridge>,
    terrain_barriers: Vec<ColliderHandle>,
    tank_fall: Option<f32>,
    flags: Vec<level::Flag>,
    stations: Vec<stations::Station>,
    team: crate::level::Team,
    winner: Option<crate::level::Team>,
}

impl World {
    /// All input and fracture mutations occur at fixed tick boundaries. Events
    /// describe this tick only; callers can record the command slice for replay.
    pub fn step(&mut self, commands: &[Command]) -> Vec<Event> {
        if self.replica.is_some() || self.winner.is_some() {
            return Vec::new();
        }
        let mut events = Vec::new();
        let drivers = profile!(self, control, {
            self.begin_tick(&mut events);
            let tracks = self.control_player(commands, true, true, &mut events);
            let mut drivers = vec![(self.launcher, tracks)];
            drivers.extend(self.enemy_commands(&mut events));
            drivers
        });
        self.finish_tick(&drivers, &mut events);
        events
    }

    fn begin_tick(&mut self, events: &mut Vec<Event>) {
        self.sight_cache = None;
        self.structural_residual = 0.0;
        self.elastic_energy = 0.0;
        self.structural_iterations = 0;
        for piece in &mut self.pieces {
            if !piece.active {
                continue;
            }
            let Some(body) = self.physics.bodies.get(piece.body) else {
                continue;
            };
            let pose = *body.position() * piece.local_pose;
            piece.previous_position = pose.translation;
            piece.previous_angle = pose.rotation.angle();
            piece.load *= 0.965;
        }
        for bond in &mut self.bonds {
            bond.load *= 0.965;
        }
        self.update_tanks(events);
        self.advance_airdrops();
        for shell in &mut self.shells {
            shell.previous_position = shell.position;
        }
    }

    fn control_player(
        &mut self,
        commands: &[Command],
        decay_reload: bool,
        advance_pickups: bool,
        events: &mut Vec<Event>,
    ) -> [f32; 2] {
        self.update_player_aim(commands);
        if decay_reload {
            self.reload = (self.reload - TICK_DT).max(0.0);
        }
        if let Some(beam) = &mut self.laser_beam {
            beam.remaining -= TICK_DT;
            if beam.remaining <= 0.0 {
                self.laser_beam = None;
            }
        }
        if let Some(beam) = &mut self.plasma_beam {
            beam.remaining -= TICK_DT;
            if beam.remaining <= 0.0 {
                self.plasma_beam = None;
            }
        }
        for command in commands {
            if let Command::SelectAmmunition(ammo) = *command {
                self.select_main_ammunition(ammo);
            }
        }
        let mut tracks = [0.0; 2];
        for command in commands {
            match *command {
                Command::Fire(shot) => {
                    if !self.player_aim.active {
                        // Precisely authored material experiments have no player controls.
                        self.fire(shot, events);
                    } else if !shot.ammunition.special() && !self.special_active() {
                        if shot.ammunition != self.primary_ammunition {
                            self.select_main_ammunition(shot.ammunition);
                        } else {
                            self.fire(shot, events);
                        }
                    }
                }
                Command::ActivateSpecial => self.activate_special(events),
                Command::Tracks { left, right } if left.is_finite() && right.is_finite() => {
                    tracks = [left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)];
                }
                _ => {}
            }
        }
        self.update_specials_for_player(events, advance_pickups);
        tracks
    }

    fn finish_tick(&mut self, drivers: &[(usize, [f32; 2])], events: &mut Vec<Event>) {
        for _ in 0..SUBSTEPS {
            let dt = self.physics.integration_parameters.dt;
            profile!(self, driving, {
                self.ground_resistance(dt);
                for &(id, tracks) in drivers {
                    self.drive_tank(id, tracks, dt);
                }
            });
            profile!(self, projectiles, {
                self.advance_shells(dt, events);
                self.advance_blasts(dt);
            });
            profile!(self, fracture, {
                self.process_loads(dt, events);
                self.detonate_objects(events);
                self.update_bridges(dt, events);
            });
            let velocities = profile!(self, contacts, { self.contact_velocities() });
            profile!(self, rapier, {
                self.physics
                    .step_with_events(&contacts::ReboundHooks(&self.pieces), &());
            });
            profile!(self, plasticity, { self.update_plastic_bends(events) });
            profile!(self, contacts, {
                let ramming = self.collect_contact_loads(&velocities, events);
                for (id, damage, impact) in ramming {
                    self.damage_tank_from(id, damage, impact, events);
                }
            });
            profile!(self, fracture, {
                self.process_loads(dt, events);
                self.detonate_objects(events);
                self.update_bridges(0.0, events);
            });
            profile!(self, cleanup, { self.capture_flags(events) });
            if self.winner.is_some() {
                break;
            }
        }
        profile!(self, cleanup, {
            if self.winner.is_none() {
                self.update_stations();
                self.update_bridge_repairs();
            }
            self.update_decorations(events);
            self.cleanup_debris();
        });
        self.tick += 1;
    }
}

#[cfg(test)]
mod ammunition_tests;
#[cfg(test)]
mod rigidity_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod terrain_tests;
