//! A horizontal destruction laboratory. Units are meters, kilograms, seconds.
//! Rigid chunks, elastic load paths, and adaptive fracture without health counters.

mod ammunition;
pub mod experiment;
pub mod level;
mod material;
mod object;
mod structure;
mod terrain;
mod world;

pub use ammunition::Ammunition;
pub use material::Material;
pub use object::{
    Components, Crushable, Explosive, GrainLayout, GrainMode, LibraryPreset, MAX_FRAGMENTS,
    ObjectAsset, ObjectLibrary, ObjectMesh, ObjectPreset, Plasticity, SoftBounce, Surface,
};
pub use rapier2d::math::Vector as Vec2;
pub use terrain::{BridgeView, TerrainKind, TerrainPatch};
pub use world::{
    BodyView, BondView, Command, Event, LaserBeam, Scene, Shape, Shell, Shot, Stats, TICK_DT,
    TankView, World,
};
