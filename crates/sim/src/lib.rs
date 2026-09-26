//! A horizontal destruction laboratory. Units are meters, kilograms, seconds.
//! Rigid chunks, elastic load paths, and adaptive fracture without health counters.

mod ammunition;
mod decoration;
pub use decoration::{DecorationKind, DecorationView};
pub mod experiment;
pub mod level;
pub mod matchplay;
mod material;
mod object;
mod structure;
mod terrain;
mod world;

pub use ammunition::{Ammunition, SpecialWeapon};
pub use material::Material;
pub use object::{
    Components, Crushable, Explosive, GrainLayout, GrainMode, LibraryPreset, MAX_FRAGMENTS,
    ObjectAsset, ObjectLibrary, ObjectMesh, ObjectPreset, Plasticity, SoftBounce, Surface,
};
pub use rapier2d::math::Vector as Vec2;
pub use terrain::{BridgeView, TerrainKind, TerrainPatch};
pub use world::{
    AimView, BodyView, BondView, Command, DestructionCause, Event, LaserBeam, NEAR_VISION,
    PickupView, Scene, Shape, Shell, Shot, StationView, Stats, TICK_DT, TankDestruction,
    TankFragment, TankImpact, TankView, VISION_HALF_ANGLE, VISION_RANGE, Vision, World,
};

pub use world::multiplayer;

pub use world::replica;
