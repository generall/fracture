//! Commands, events and read-only views at the simulation boundary.
use super::{Ammunition, Material, Vector};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scene {
    #[default]
    Masonry,
    Cantilever,
    LooseBlocks,
    Custom,
    ObjectYard,
    Terrain,
    Level,
    BarrelRubble,
}

impl Scene {
    pub const ALL: [Self; 3] = [Self::Masonry, Self::Cantilever, Self::LooseBlocks];
    pub fn title(self) -> &'static str {
        match self {
            Self::Masonry => "Masonry barrier",
            Self::Cantilever => "The weak link",
            Self::LooseBlocks => "Rock field",
            Self::Custom => "Your object",
            Self::Level => "Your level",
            Self::ObjectYard => "Object yard",
            Self::Terrain => "River crossing",
            Self::BarrelRubble => "Barrel against rubble",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Masonry => "Breach the barrier.",
            Self::Cantilever => "Break the neck. Bring it down.",
            Self::LooseBlocks => "Scatter the field.",
            Self::ObjectYard => {
                "Object library. Pin green barrels against the backstop; red barrels explode."
            }
            Self::Terrain => {
                "Road / sand / water. Cross the bridge; target the booths marked A / B."
            }
            Self::Custom => "Custom object test. O to return to the editor. R to rebuild.",
            Self::Level => "Capture the enemy flag. M to edit the map. R to restart.",
            Self::BarrelRubble => "Hold W to squeeze the barrel into rubble. R to reset.",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Shot {
    pub ammunition: Ammunition,
    pub target: Vector,
    pub mass: f32,
    pub speed: f32,
}

impl Shot {
    pub fn new(ammunition: Ammunition, target: Vector) -> Self {
        let (mass, speed) = ammunition.defaults();
        Self {
            ammunition,
            target,
            mass,
            speed,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Command {
    Deploy(crate::VehicleKind),
    Fire(Shot),
    SelectAmmunition(Ammunition),
    ActivateSpecial,
    /// Desired player look point; the physical turret turns toward it over time.
    Aim(Vector),
    /// Left and right track throttles in [-1, 1]. Zero engages the brakes.
    /// Differential tracks for tracked vehicles. For scouts, left + right is
    /// throttle and left - right is front-wheel steering (each clamped to ±1).
    Tracks {
        left: f32,
        right: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DestructionCause {
    Shell(Ammunition),
    Blast,
    Ram,
    Laser,
    Collapse,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TankImpact {
    pub cause: DestructionCause,
    pub point: Vector,
    /// Direction of the incoming force, from the impact toward the tank.
    pub direction: Vector,
    /// Impulse not yet applied by the solver (ramming is already resolved).
    pub impulse: Vector,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TankDestruction {
    pub vehicle: crate::VehicleKind,
    pub mounted: bool,
    pub id: usize,
    pub hull_angle: f32,
    pub turret_angle: f32,
    pub velocity: Vector,
    pub impact: TankImpact,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Fired {
        ammunition: Ammunition,
        position: Vector,
        velocity: Vector,
    },
    Fractured {
        position: Vector,
        normal: Vector,
    },
    Exploded {
        position: Vector,
        radius: f32,
    },
    MuzzleBlocked,
    Impacted {
        position: Vector,
        normal: Vector,
    },
    /// Significant hull contact; presentation may rate-limit the resulting sound.
    Collided {
        position: Vector,
        energy: f32,
    },
    /// Permanent shell buckling (e.g. a crushed barrel); work is joules.
    Buckled {
        position: Vector,
        work: f32,
    },
    /// A metal hinge yielded and retained a bend; work is joules.
    Bent {
        position: Vector,
        work: f32,
    },
    /// A nonphysical scenery prop was destroyed by a weapon or tank contact.
    DecorationDestroyed {
        position: Vector,
        kind: crate::DecorationKind,
    },
    LaserLimited,
    BridgeCollapsed {
        position: Vector,
    },
    TankDestroyed {
        position: Vector,
        team: crate::level::Team,
        details: TankDestruction,
    },
    TankRespawned {
        position: Vector,
        team: crate::level::Team,
    },
    TankSunk {
        position: Vector,
    },
    FlagCaptured {
        position: Vector,
        team: crate::level::Team,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Shape {
    Box { half: Vector },
    Ball { radius: f32 },
    Triangle { vertices: [Vector; 3] },
}

/// Authored detail retained on an independent tank wreck body.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TankFragment {
    /// 0..4 are hull plates; 4..10 are track sections.
    pub part: u8,
    pub local_offset: Vector,
    pub tread_phase: f32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct BodyView {
    /// Elevated bridge mesh, drawn as a continuous deck during normal play.
    #[serde(default)]
    pub bridge_deck: bool,
    pub id: usize,
    /// Authored object shared by its material cells, including fixed polygons.
    pub object: Option<usize>,
    /// Cells in the same chunk share exactly one physical transform.
    pub chunk: u32,
    pub origin: usize,
    pub position: Vector,
    pub previous_position: Vector,
    pub angle: f32,
    pub previous_angle: f32,
    pub velocity: Vector,
    pub shape: Shape,
    pub shape_revision: u32,
    pub tank_fragment: Option<TankFragment>,
    pub plastic_strain: f32,
    /// Visible triangle edges: grain boundaries and exposed fracture surfaces.
    pub edges: u8,
    pub material: Material,
    pub color: Option<[u8; 3]>,
    pub load: f32,
    pub sleeping: bool,
    pub fixed: bool,
    pub opacity: f32,
    /// Host sight-blocker classification, filled only in network snapshots so
    /// replicas can compute their own vision without physics colliders.
    pub blocks_sight: bool,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct BondView {
    pub start: Vector,
    pub end: Vector,
    pub position: Vector,
    pub normal: Vector,
    pub width: f32,
    pub load: f32,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Stats {
    pub ablated: usize,
    pub ablated_area: f32,
    pub plastic_work: f64,
    pub plastic_strain: f32,
    pub ticks: u64,
    pub bodies: usize,
    pub awake: usize,
    pub bonds: usize,
    pub broken: usize,
    pub shots: usize,
    pub shells: usize,
    pub explosions: usize,
    pub cleared: usize,
    pub cleared_area: f32,
    pub kinetic_energy: f32,
    pub peak_load: f32,
    pub cells: usize,
    pub chunks: usize,
    pub refinements: usize,
    pub refinement_limited: usize,
    pub fracture_work: f64,
    pub structural_residual: f64,
    pub elastic_energy: f64,
    pub structural_iterations: usize,
    pub unconverged: usize,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Shell {
    pub vehicle: crate::VehicleKind,
    /// Total flight time for arcing mortar rounds; zero for direct fire.
    pub flight: f32,
    pub ammunition: Ammunition,
    pub position: Vector,
    pub previous_position: Vector,
    pub velocity: Vector,
    pub mass: f32,
    pub age: f32,
    pub(super) owner: usize,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaserBeam {
    pub start: Vector,
    pub end: Vector,
    pub remaining: f32,
}
