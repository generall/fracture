//! Read-only browser diagnostics for loading detection and smoke tests.
//! These exports never modify simulation state.
use std::sync::atomic::{AtomicU32, Ordering};

static TICKS: AtomicU32 = AtomicU32::new(0);
static BROKEN: AtomicU32 = AtomicU32::new(0);
static SHOTS: AtomicU32 = AtomicU32::new(0);
static BODIES: AtomicU32 = AtomicU32::new(0);
static CELLS: AtomicU32 = AtomicU32::new(0);
static CHUNKS: AtomicU32 = AtomicU32::new(0);
static REFINED: AtomicU32 = AtomicU32::new(0);
static UNCONVERGED: AtomicU32 = AtomicU32::new(0);
static CLEARED_AREA: AtomicU32 = AtomicU32::new(0);
static SPECIAL: AtomicU32 = AtomicU32::new(0);
static PLASMA_TIME: AtomicU32 = AtomicU32::new(0);
static LASER_TIME: AtomicU32 = AtomicU32::new(0);
static RELOAD: AtomicU32 = AtomicU32::new(0);
static AMMUNITION: AtomicU32 = AtomicU32::new(0);
static TURRET_ANGLE: AtomicU32 = AtomicU32::new(0);
static ABLATED: AtomicU32 = AtomicU32::new(0);
static ABLATED_AREA: AtomicU32 = AtomicU32::new(0);

pub fn publish_ammunition(ammunition: sim::Ammunition) {
    AMMUNITION.store(ammunition as u32, Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_ammunition() -> u32 {
    AMMUNITION.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_ablated() -> u32 {
    ABLATED.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_ablated_area() -> f32 {
    f32::from_bits(ABLATED_AREA.load(Ordering::Relaxed))
}

#[unsafe(no_mangle)]
pub extern "C" fn lab_special() -> u32 {
    SPECIAL.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_laser_remaining() -> f32 {
    f32::from_bits(LASER_TIME.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_plasma_remaining() -> f32 {
    f32::from_bits(PLASMA_TIME.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_reload() -> f32 {
    f32::from_bits(RELOAD.load(Ordering::Relaxed))
}

pub fn publish(world: &sim::World) {
    TURRET_ANGLE.store(world.player_aim().angle.to_bits(), Ordering::Relaxed);
    SPECIAL.store(
        world.carried_special().map_or(0, |w| match w {
            sim::SpecialWeapon::Laser => 1,
            sim::SpecialWeapon::Rocket => 2,
            sim::SpecialWeapon::Plasma => 3,
        }),
        Ordering::Relaxed,
    );
    PLASMA_TIME.store(world.plasma_remaining().to_bits(), Ordering::Relaxed);
    LASER_TIME.store(world.laser_remaining().to_bits(), Ordering::Relaxed);
    RELOAD.store(world.reload_remaining().to_bits(), Ordering::Relaxed);
    WINNER.store(
        world
            .winner()
            .map_or(0, |t| if t == sim::level::Team::Blue { 1 } else { 2 }),
        Ordering::Relaxed,
    );
    FLAGS.store(world.flags().count() as u32, Ordering::Relaxed);
    let stats = world.stats();
    TANK_ALIVE.store(u32::from(world.tank_alive()), Ordering::Relaxed);
    BRIDGES.store(
        world.bridges().filter(|b| b.intact).count() as u32,
        Ordering::Relaxed,
    );
    TERRAIN.store(
        world.terrain_at(world.launcher_position()) as u32,
        Ordering::Relaxed,
    );
    ABLATED.store(stats.ablated as u32, Ordering::Relaxed);
    ABLATED_AREA.store(stats.ablated_area.to_bits(), Ordering::Relaxed);
    PLASTIC_STRAIN.store(stats.plastic_strain.to_bits(), Ordering::Relaxed);
    PLASTIC_WORK.store((stats.plastic_work as f32).to_bits(), Ordering::Relaxed);
    CELLS.store(stats.cells as u32, Ordering::Relaxed);
    CHUNKS.store(stats.chunks as u32, Ordering::Relaxed);
    REFINED.store(stats.refinements as u32, Ordering::Relaxed);
    UNCONVERGED.store(stats.unconverged as u32, Ordering::Relaxed);
    CLEARED_AREA.store(stats.cleared_area.to_bits(), Ordering::Relaxed);
    SHELLS.store(stats.shells as u32, Ordering::Relaxed);
    EXPLOSIONS.store(stats.explosions as u32, Ordering::Relaxed);
    CLEARED.store(stats.cleared as u32, Ordering::Relaxed);
    TANK_X.store(world.launcher_position().x.to_bits(), Ordering::Relaxed);
    TANK_Y.store(world.launcher_position().y.to_bits(), Ordering::Relaxed);
    let angle = world
        .bodies()
        .find(|b| b.material == sim::Material::Launcher)
        .unwrap()
        .angle;
    TANK_ANGLE.store(angle.to_bits(), Ordering::Relaxed);
    TICKS.store(stats.ticks as u32, Ordering::Relaxed);
    BROKEN.store(stats.broken as u32, Ordering::Relaxed);
    SHOTS.store(stats.shots as u32, Ordering::Relaxed);
    BODIES.store(stats.bodies as u32, Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_turret_angle() -> f32 {
    f32::from_bits(TURRET_ANGLE.load(Ordering::Relaxed))
}
static PLASTIC_STRAIN: AtomicU32 = AtomicU32::new(0);
static PLASTIC_WORK: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn lab_plastic_strain() -> f32 {
    f32::from_bits(PLASTIC_STRAIN.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_plastic_work() -> f32 {
    f32::from_bits(PLASTIC_WORK.load(Ordering::Relaxed))
}

#[unsafe(no_mangle)]
pub extern "C" fn lab_ticks() -> u32 {
    TICKS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_broken() -> u32 {
    BROKEN.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_shots() -> u32 {
    SHOTS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_bodies() -> u32 {
    BODIES.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_cells() -> u32 {
    CELLS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_chunks() -> u32 {
    CHUNKS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_refinements() -> u32 {
    REFINED.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_unconverged() -> u32 {
    UNCONVERGED.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_cleared_area() -> f32 {
    f32::from_bits(CLEARED_AREA.load(Ordering::Relaxed))
}

static SHELLS: AtomicU32 = AtomicU32::new(0);
static EXPLOSIONS: AtomicU32 = AtomicU32::new(0);
static CLEARED: AtomicU32 = AtomicU32::new(0);
static TANK_X: AtomicU32 = AtomicU32::new(0);
static TANK_Y: AtomicU32 = AtomicU32::new(0);
static TANK_ANGLE: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn lab_shells() -> u32 {
    SHELLS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_explosions() -> u32 {
    EXPLOSIONS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_cleared() -> u32 {
    CLEARED.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_tank_x() -> f32 {
    f32::from_bits(TANK_X.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_tank_y() -> f32 {
    f32::from_bits(TANK_Y.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_tank_angle() -> f32 {
    f32::from_bits(TANK_ANGLE.load(Ordering::Relaxed))
}

static ZOOM: AtomicU32 = AtomicU32::new(0);
static CAMERA_X: AtomicU32 = AtomicU32::new(0);
static CAMERA_Y: AtomicU32 = AtomicU32::new(0);
pub fn publish_camera(camera: &crate::navigation::Navigation) {
    ZOOM.store(camera.zoom.to_bits(), Ordering::Relaxed);
    CAMERA_X.store(camera.center.x.to_bits(), Ordering::Relaxed);
    CAMERA_Y.store(camera.center.y.to_bits(), Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_zoom() -> f32 {
    f32::from_bits(ZOOM.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_camera_x() -> f32 {
    f32::from_bits(CAMERA_X.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_camera_y() -> f32 {
    f32::from_bits(CAMERA_Y.load(Ordering::Relaxed))
}

static TANK_ALIVE: AtomicU32 = AtomicU32::new(1);
static BRIDGES: AtomicU32 = AtomicU32::new(0);
static TERRAIN: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn lab_tank_alive() -> u32 {
    TANK_ALIVE.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_bridges() -> u32 {
    BRIDGES.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_terrain() -> u32 {
    TERRAIN.load(Ordering::Relaxed)
}

static WINNER: AtomicU32 = AtomicU32::new(0);
static FLAGS: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
pub extern "C" fn lab_winner() -> u32 {
    WINNER.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn lab_flags() -> u32 {
    FLAGS.load(Ordering::Relaxed)
}
