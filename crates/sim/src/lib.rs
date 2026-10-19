//! A horizontal destruction laboratory. Units are meters, kilograms, seconds.
//! Rigid chunks, elastic load paths, and adaptive fracture without health counters.

pub mod ai;
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
mod vehicle;
mod world;
pub use vehicle::VehicleKind;

pub use ammunition::{Ammunition, SpecialWeapon};
pub use material::Material;
pub use object::{
    Components, Crushable, Explosive, GrainLayout, GrainMode, LibraryPreset, MAX_FRAGMENTS,
    ObjectAsset, ObjectLibrary, ObjectMesh, ObjectPreset, Plasticity, SoftBounce, Surface,
};
pub use rapier2d::math::Vector as Vec2;
pub use terrain::{
    BRIDGE_REPAIR_RADIUS, BRIDGE_REPAIR_SECONDS, BridgeRepairView, BridgeView, TerrainKind,
    TerrainPatch,
};
pub use world::{
    AiGeometry, AimView, AirdropView, BodyView, BondView, Command, DestructionCause, Event,
    LaserBeam, MineView, NEAR_VISION, PickupView, RESPAWN_SECONDS, Scene, Shape, Shell, Shot,
    StationView, Stats, TICK_DT, TankDestruction, TankFragment, TankImpact, TankView, TurretView,
    VISION_HALF_ANGLE, VISION_RANGE, Vision, World,
};

pub use world::multiplayer;

#[cfg(feature = "physics-profile")]
pub use world::PhysicsProfile;
pub use world::replica;
