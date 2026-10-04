//! Presentation snapshots. A replica has no physics bodies and cannot simulate.
use super::*;
use crate::{
    BridgeView, DecorationView, TerrainPatch,
    level::{Team, TerrainMask},
};
use serde::{Deserialize, Serialize};

/// Immutable map data is transferred once per match, outside periodic updates.
#[derive(Clone, Serialize, Deserialize)]
pub struct Map {
    pub size: Vector,
    pub terrain: Vec<TerrainPatch>,
    pub mask: Option<TerrainMask>,
    pub flags: Vec<(Team, Vector)>,
}
impl Map {
    pub fn capture(world: &World) -> Self {
        Self {
            size: world.arena_size,
            terrain: world.terrain.clone(),
            mask: world.terrain_mask.clone(),
            flags: world.flags().collect(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Bridge {
    pub deck: TerrainPatch,
    pub controls: [Vector; 2],
    pub intact: bool,
    pub fall_progress: f32,
    pub fragments: Vec<BodyView>,
    pub repairs: [crate::BridgeRepairView; 2],
}
impl Bridge {
    pub fn view(&self) -> BridgeView<'_> {
        BridgeView {
            deck: self.deck,
            controls: self.controls,
            intact: self.intact,
            fall_progress: self.fall_progress,
            fragments: &self.fragments,
            repairs: self.repairs,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct WeaponEffect {
    pub tank: usize,
    pub muzzle: Vector,
    pub laser: Option<LaserBeam>,
    pub plasma: Option<LaserBeam>,
    pub charging: f32,
    pub laser_elapsed: f32,
}
/// Dynamic presentation state. The transport encodes body definitions and motion
/// separately, sends only changed records, and filters each player's interest set.
#[derive(Clone, Serialize, Deserialize)]
pub struct Frame {
    pub bodies: Vec<BodyView>,
    pub parents: std::collections::BTreeMap<usize, usize>,
    pub weapons: Vec<WeaponEffect>,
    pub tanks: Vec<TankView>,
    pub shells: Vec<Shell>,
    pub bridges: Vec<Bridge>,
    pub decorations: Vec<DecorationView>,
    pub bursts: Vec<(DecorationView, f32)>,
    pub pickups: Vec<PickupView>,
    pub airdrop: Option<AirdropView>,
    pub stations: Vec<StationView>,
    pub stats: Stats,
    pub player: usize,
    pub team: Team,
    pub winner: Option<Team>,
    pub position: Vector,
    pub aim: AimView,
    pub tracks: [f32; 2],
    pub track_travel: [f32; 2],
    pub reload: f32,
    pub ammunition: Ammunition,
    pub special: Option<crate::SpecialWeapon>,
    pub charges: u32,
    pub cooldown: f32,
    pub laser_remaining: f32,
    pub plasma_remaining: f32,
    pub laser: Option<LaserBeam>,
    pub plasma: Option<LaserBeam>,
    pub alive: bool,
    pub respawn: Option<f32>,
    pub fall: f32,
    pub sight_range: f32,
    pub radar: bool,
    pub vision: Vision,
}
impl Frame {
    /// Trusted clients receive every body and compute their own sight geometry.
    pub fn capture(world: &World) -> Self {
        let blockers = world.sight_blockers();
        Self {
            bodies: world
                .bodies()
                .map(|mut b| {
                    b.blocks_sight = blockers.contains(&b.id);
                    b
                })
                .collect(),
            parents: world
                .pieces
                .iter()
                .enumerate()
                .filter_map(|(id, p)| p.fracture_parent.map(|parent| (id, parent)))
                .collect(),
            weapons: world.weapon_effects().collect(),
            tanks: world.tanks().collect(),
            shells: world.shells().copied().collect(),
            bridges: world
                .bridges()
                .map(|b| Bridge {
                    deck: b.deck,
                    controls: b.controls,
                    intact: b.intact,
                    fall_progress: b.fall_progress,
                    fragments: b.fragments.to_vec(),
                    repairs: b.repairs,
                })
                .collect(),
            decorations: world.decorations().collect(),
            bursts: world.decoration_bursts().collect(),
            pickups: world.pickups().collect(),
            airdrop: world.airdrop(),
            stations: world.stations().collect(),
            stats: world.stats(),
            player: world.player_tank_id(),
            team: world.team(),
            winner: world.winner(),
            position: world.launcher_position(),
            aim: world.player_aim(),
            tracks: world.track_speeds(),
            track_travel: world.track_travel(),
            reload: world.reload_remaining(),
            ammunition: world.last_ammunition(),
            special: world.carried_special(),
            charges: world.special_charges(),
            cooldown: world.special_cooldown(),
            laser_remaining: world.laser_remaining(),
            plasma_remaining: world.plasma_remaining(),
            laser: world.laser_beam(),
            plasma: world.plasma_beam(),
            alive: world.tank_alive(),
            respawn: world.player_respawn_remaining(),
            fall: world.tank_fall_progress(),
            sight_range: world.player_sight_range(),
            radar: world.player_in_radar_zone(),
            vision: world.host_vision(world.player_aim().angle),
        }
    }
    /// Shared geometry, scenery and diagnostics are captured once per host update.
    pub fn for_player(world: &World, shared: &Self) -> Self {
        let mut frame = shared.clone();
        frame.player = world.player_tank_id();
        frame.team = world.team();
        frame.position = world.launcher_position();
        frame.aim = world.player_aim();
        frame.tracks = world.track_speeds();
        frame.track_travel = world.track_travel();
        frame.reload = world.reload_remaining();
        frame.ammunition = world.last_ammunition();
        frame.special = world.carried_special();
        frame.charges = world.special_charges();
        frame.cooldown = world.special_cooldown();
        frame.laser_remaining = world.laser_remaining();
        frame.plasma_remaining = world.plasma_remaining();
        frame.laser = world.laser_beam();
        frame.plasma = world.plasma_beam();
        frame.alive = world.tank_alive();
        frame.respawn = world.player_respawn_remaining();
        frame.fall = world.tank_fall_progress();
        frame.sight_range = world.player_sight_range();
        frame.radar = world.player_in_radar_zone();
        frame.vision = world.host_vision(frame.aim.angle);
        frame
    }
    /// Interpolation endpoints are consecutive network snapshots, not the host's
    /// previous 120 Hz physics subframe. Newly visible bodies appear in place.
    pub fn interpolate_from(&mut self, previous: &Self) {
        use std::collections::BTreeMap;
        let bodies: BTreeMap<_, _> = previous.bodies.iter().map(|b| (b.id, b)).collect();
        for body in &mut self.bodies {
            let old = bodies
                .get(&body.id)
                .copied()
                .filter(|b| b.position.distance_squared(body.position) < 100.0);
            body.previous_position = old.map_or(body.position, |b| b.position);
            body.previous_angle = old.map_or(body.angle, |b| b.angle);
        }
        for tank in &mut self.tanks {
            tank.previous_turret_angle = previous
                .tanks
                .iter()
                .find(|t| t.id == tank.id)
                .map_or(tank.turret_angle, |t| t.turret_angle);
        }
        self.aim.previous_angle = previous.aim.angle;
        // Shell IDs are host-owned; age identifies successive shots from one owner.
        let elapsed = self.stats.ticks.saturating_sub(previous.stats.ticks) as f32 * TICK_DT;
        for shell in &mut self.shells {
            shell.previous_position = shell.position - shell.velocity * elapsed.min(shell.age);
        }
    }
}
impl World {
    pub fn weapon_effects(&self) -> impl Iterator<Item = WeaponEffect> + '_ {
        std::iter::once(())
            .filter(|_| self.replica.is_none())
            .map(|_| WeaponEffect {
                tank: self.launcher,
                muzzle: self.launcher_position() + Vector::from_angle(self.player_aim.angle) * 1.65,
                laser: self.laser_beam,
                plasma: self.plasma_beam,
                charging: if self.tank_alive() {
                    self.plasma_remaining
                } else {
                    0.0
                },
                laser_elapsed: 3.0 - self.laser_remaining,
            })
            .chain(self.replica.iter().flat_map(|f| f.weapons.iter().copied()))
    }
    pub fn from_replica(map: Map, frame: Frame) -> Self {
        let mut world = Self::new(Scene::Level);
        world.physics = PhysicsWorld::default();
        world.pieces.clear();
        world.bonds.clear();
        world.incident_bonds.clear();
        world.arena_size = map.size;
        world.terrain = map.terrain;
        world.terrain_mask = map.mask;
        world.flags = map
            .flags
            .into_iter()
            .map(|(team, position)| level::Flag { team, position })
            .collect();
        world.apply_replica(frame);
        world
    }
    pub fn is_replica(&self) -> bool {
        self.replica.is_some()
    }
    pub fn apply_replica(&mut self, frame: Frame) {
        assert!(
            self.replica.is_some() || self.pieces.is_empty(),
            "Cannot replace a simulation with remote state"
        );
        self.launcher = frame.player;
        self.team = frame.team;
        self.winner = frame.winner;
        self.tick = frame.stats.ticks;
        self.reload = frame.reload;
        self.track_travel = frame.track_travel;
        self.last_ammunition = frame.ammunition;
        self.special = frame.special;
        self.special_charges = frame.charges;
        self.special_reload = frame.cooldown;
        self.laser_remaining = frame.laser_remaining;
        self.plasma_remaining = frame.plasma_remaining;
        self.laser_beam = frame.laser;
        self.plasma_beam = frame.plasma;
        self.replica = Some(Box::new(frame));
        self.apply_replica_vision();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolation_uses_network_endpoints_and_snaps_respawn_teleports() {
        let world = World::from_level(&crate::level::Level::default(), Team::Blue).unwrap();
        let before = Frame::capture(&world);
        let mut after = before.clone();
        after.stats.ticks += 6;
        let player = after
            .bodies
            .iter_mut()
            .find(|b| b.id == after.player)
            .unwrap();
        let start = player.position;
        player.position += Vector::X;
        player.previous_position = player.position - Vector::X * 0.01;
        after.interpolate_from(&before);
        let player = after.bodies.iter().find(|b| b.id == after.player).unwrap();
        assert_eq!(player.previous_position, start);
        assert_eq!(player.position, start + Vector::X);
        let mut respawn = after.clone();
        respawn
            .bodies
            .iter_mut()
            .find(|b| b.id == respawn.player)
            .unwrap()
            .position += Vector::X * 25.0;
        respawn.interpolate_from(&after);
        let player = respawn
            .bodies
            .iter()
            .find(|b| b.id == respawn.player)
            .unwrap();
        assert_eq!(player.previous_position, player.position);
    }
    #[test]
    fn replicas_have_no_physics_and_ignore_simulation_commands() {
        let world = World::from_level(&crate::level::Level::default(), Team::Blue).unwrap();
        let mut replica = World::from_replica(Map::capture(&world), Frame::capture(&world));
        assert_eq!(replica.physics.bodies.len(), 0);
        assert_eq!(replica.bodies().count(), world.bodies().count());
        assert_eq!(replica.tanks().count(), 2);
        assert_eq!(replica.launcher_position(), world.launcher_position());
        assert_eq!(replica.track_speeds(), world.track_speeds());
        assert_eq!(replica.player_respawn_remaining(), None);
        assert_eq!(
            replica.player_vision(1.0).boundary,
            world.player_vision(world.player_aim().angle).boundary
        );
        assert!(
            replica
                .step(&[Command::Tracks {
                    left: 1.0,
                    right: 1.0
                }])
                .is_empty()
        );
        assert_eq!(replica.stats().ticks, 0);
    }
}
