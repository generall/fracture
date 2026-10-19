//! Host-only multi-tank control. Physics advances exactly once per world tick.
use super::*;
use crate::{
    level::{Level, LevelItem, Team},
    matchplay::MatchConfig,
};

pub const MAX_PLAYERS: usize = 6;

/// Controls contain intent only: mass, velocity, health and positions are host-owned.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlayerInput {
    /// Drive channels, using the same vehicle-specific encoding as Command::Tracks.
    pub tracks: [f32; 2],
    pub aim: Vector,
    pub ammunition: Ammunition,
    pub fire: bool,
    pub special: bool,
    pub deploy: Option<VehicleKind>,
}
impl Default for PlayerInput {
    fn default() -> Self {
        Self {
            tracks: [0.0; 2],
            aim: Vector::X,
            ammunition: Ammunition::HighExplosive,
            fire: false,
            special: false,
            deploy: None,
        }
    }
}
impl PlayerInput {
    pub fn valid(&self) -> bool {
        self.aim.is_finite()
            && self.aim.abs().max_element() <= 10000.0
            && self.tracks.iter().all(|v| v.is_finite() && v.abs() <= 1.0)
            && Ammunition::MAIN.contains(&self.ammunition)
    }
    pub(super) fn commands(self) -> Vec<Command> {
        let mut commands = vec![
            Command::SelectAmmunition(self.ammunition),
            Command::Aim(self.aim),
            Command::Tracks {
                left: self.tracks[0],
                right: self.tracks[1],
            },
        ];
        if let Some(kind) = self.deploy {
            commands.push(Command::Deploy(kind));
        }
        if self.fire {
            commands.push(Command::Fire(Shot::new(self.ammunition, self.aim)));
        }
        if self.special {
            commands.push(Command::ActivateSpecial);
        }
        commands
    }
}

// Existing weapon routines operate on the selected tank. Store all of their
// per-player state here while keeping bodies, damage and world timers shared.
#[derive(Clone, Copy)]
pub(super) struct Weapons {
    primary: Ammunition,
    special: Option<crate::SpecialWeapon>,
    charges: u32,
    interval: f32,
    cooldown: f32,
    laser: f32,
    plasma: f32,
    direction: Vector,
    pulse: f32,
    last: Ammunition,
    beam: Option<LaserBeam>,
    plasma_beam: Option<LaserBeam>,
}
impl Weapons {
    pub(super) fn capture(w: &World) -> Self {
        Self {
            primary: w.primary_ammunition,
            special: w.special,
            charges: w.special_charges,
            interval: w.special_interval,
            cooldown: w.special_reload,
            laser: w.laser_remaining,
            plasma: w.plasma_remaining,
            direction: w.plasma_direction,
            pulse: w.laser_pulse_in,
            last: w.last_ammunition,
            beam: w.laser_beam,
            plasma_beam: w.plasma_beam,
        }
    }
    pub(super) fn apply(self, w: &mut World) {
        w.primary_ammunition = self.primary;
        w.special = self.special;
        w.special_charges = self.charges;
        w.special_interval = self.interval;
        w.special_reload = self.cooldown;
        w.laser_remaining = self.laser;
        w.plasma_remaining = self.plasma;
        w.plasma_direction = self.direction;
        w.laser_pulse_in = self.pulse;
        w.last_ammunition = self.last;
        w.laser_beam = self.beam;
        w.plasma_beam = self.plasma_beam;
    }
}
impl Default for Weapons {
    fn default() -> Self {
        Self {
            primary: Ammunition::HighExplosive,
            special: None,
            charges: 0,
            interval: 0.0,
            cooldown: 0.0,
            laser: 0.0,
            plasma: 0.0,
            direction: Vector::X,
            pulse: 0.0,
            last: Ammunition::HighExplosive,
            beam: None,
            plasma_beam: None,
        }
    }
}
struct Player {
    id: usize,
    team: Team,
    weapons: Weapons,
    connected: bool,
    shots: usize,
}

pub struct MultiplayerWorld {
    world: World,
    players: Vec<Player>,
}
impl MultiplayerWorld {
    pub fn new(level: &Level, teams: &[Team]) -> Result<Self, String> {
        if !(2..=MAX_PLAYERS).contains(&teams.len())
            || !teams.contains(&Team::Blue)
            || !teams.contains(&Team::Red)
            || teams.contains(&Team::Neutral)
        {
            return Err("A LAN match needs 2–6 players with at least one on each team.".into());
        }
        let mut world = MatchConfig {
            team: teams[0],
            ..Default::default()
        }
        .create_world(level)?;
        world.enemy = None;
        let weapons = Weapons::capture(&world);
        let mut players: Vec<Player> = Vec::new();
        for &team in teams {
            let id = if let Some(tank) = world.tanks.iter().find(|t| {
                t.mount.is_none() && t.team == team && !players.iter().any(|p| p.id == t.id)
            }) {
                tank.id
            } else {
                let (spawn, angle) = level
                    .items
                    .iter()
                    .find_map(|item| match item {
                        LevelItem::Spawn {
                            team: t,
                            position,
                            angle,
                        } if *t == team => Some((Vector::from(*position), *angle)),
                        _ => None,
                    })
                    .ok_or("Team spawn missing")?;
                let id = world.add_tank();
                world
                    .physics
                    .bodies
                    .propagate_modified_body_positions_to_colliders(&mut world.physics.colliders);
                let position = (0..129)
                    .find_map(|n| {
                        let position = if n == 0 {
                            spawn
                        } else {
                            let radius = ((n - 1) / 16 + 1) as f32 * 2.8;
                            spawn
                                + Vector::from_angle((n - 1) as f32 * std::f32::consts::TAU / 16.0)
                                    * radius
                        };
                        world
                            .tank_pose_clear(id, Pose::new(position, angle))
                            .then_some(position)
                    })
                    .ok_or_else(|| {
                        format!(
                            "Not enough clear space around the {} spawn for this team.",
                            team.label()
                        )
                    })?;
                world.register_tank(id, team, position, angle);
                world
                    .physics
                    .bodies
                    .propagate_modified_body_positions_to_colliders(&mut world.physics.colliders);
                id
            };
            players.push(Player {
                id,
                team,
                weapons,
                connected: true,
                shots: 0,
            });
        }
        let mut result = Self { world, players };
        result.activate(0);
        Ok(result)
    }
    pub fn require_vehicle_choices(&mut self) {
        for player in &self.players {
            self.world.require_vehicle_choice(player.id);
        }
    }
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
    pub fn configure_airdrops(&mut self, enabled: bool, seed: u64) {
        self.world.configure_airdrops(enabled, seed);
    }
    pub fn shots(&self, player: usize) -> usize {
        self.players.get(player).map_or(0, |p| p.shots)
    }
    pub fn tank_id(&self, player: usize) -> Option<usize> {
        self.players.get(player).map(|p| p.id)
    }
    /// Reuse blocker classification, collider bounds and composed foliage across all player snapshots.
    pub fn prepare_snapshot(&mut self) {
        self.world.prepare_snapshot_vision();
    }
    pub fn weapon_effects(&self) -> Vec<replica::WeaponEffect> {
        self.players
            .iter()
            .filter(|p| self.world.tank_is_alive(p.id))
            .map(|p| {
                let tank = self.world.tanks.iter().find(|t| t.id == p.id).unwrap();
                replica::WeaponEffect {
                    tank: p.id,
                    muzzle: self.world.piece_pose(p.id).translation
                        + Vector::from_angle(tank.aim.angle) * 1.65,
                    laser: p.weapons.beam,
                    plasma: p.weapons.plasma_beam,
                    charging: p.weapons.plasma,
                    laser_elapsed: 3.0 - p.weapons.laser,
                }
            })
            .collect()
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    #[cfg(feature = "physics-profile")]
    pub fn take_physics_profile(&mut self) -> super::PhysicsProfile {
        self.world.take_physics_profile()
    }

    fn activate(&mut self, player: usize) {
        let p = &self.players[player];
        p.weapons.apply(&mut self.world);
        self.world.launcher = p.id;
        self.world.team = p.team;
        self.world.tank_fall = None;
        let tank = self.world.tanks.iter().find(|t| t.id == p.id).unwrap();
        self.world.player_aim = tank.aim;
        self.world.reload = tank.reload;
        self.world.track_travel = tank.track_travel;
    }
    fn save(&mut self, player: usize) {
        self.players[player].weapons = Weapons::capture(&self.world);
        let tank = self
            .world
            .tanks
            .iter_mut()
            .find(|t| t.id == self.world.launcher)
            .unwrap();
        tank.reload = self.world.reload;
        tank.aim = self.world.player_aim;
    }
    /// The returned view is selected for this player; no simulation is run here.
    pub fn for_player(&mut self, player: usize) -> Option<&World> {
        if player >= self.players.len() {
            return None;
        }
        self.activate(player);
        Some(&self.world)
    }
    pub fn step(&mut self, inputs: &[PlayerInput]) -> Vec<Event> {
        if self.world.winner.is_some() {
            return vec![];
        }
        self.activate(0);
        let mut events = Vec::new();
        self.world.begin_tick(&mut events);
        let mut drivers = Vec::with_capacity(self.players.len());
        for i in 0..self.players.len() {
            self.activate(i);
            let commands = inputs
                .get(i)
                .filter(|input| input.valid() && self.players[i].connected)
                .copied()
                .map(PlayerInput::commands)
                .unwrap_or_default();
            let before = self.world.shots;
            let tracks = self
                .world
                .control_player(&commands, false, i == 0, &mut events);
            self.players[i].shots += self.world.shots.saturating_sub(before);
            self.save(i);
            drivers.push((self.players[i].id, tracks));
        }
        self.activate(0);
        self.world.finish_tick(&drivers, &mut events);
        self.save(0);
        events
    }
    pub fn disconnect(&mut self, player: usize) {
        let Some(p) = self.players.get_mut(player) else {
            return;
        };
        p.connected = false;
        let tank = self.world.tanks.iter_mut().find(|t| t.id == p.id).unwrap();
        tank.health = 0.0;
        tank.respawn = None;
        self.world.physics.bodies[self.world.pieces[p.id].body].set_enabled(false);
        let blue = self
            .players
            .iter()
            .any(|p| p.connected && p.team == Team::Blue);
        let red = self
            .players
            .iter()
            .any(|p| p.connected && p.team == Team::Red);
        if blue != red {
            self.world.winner = Some(if blue { Team::Blue } else { Team::Red });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mines_use_remote_inventory_and_each_teams_snapshot() {
        let mut level = Level::default();
        level.items.push(LevelItem::Pickup {
            weapon: crate::SpecialWeapon::Mine,
            position: [18.0, 0.0],
            respawn_time: 60.0,
            charges: 2,
            cooldown: 1.0,
        });
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red, Team::Red]).unwrap();
        game.step(&[PlayerInput::default(); 3]);
        assert_eq!(game.for_player(1).unwrap().special_charges(), 2);
        let mut inputs = [PlayerInput::default(); 3];
        inputs[1].special = true;
        game.step(&inputs);
        assert_eq!(game.for_player(1).unwrap().special_charges(), 1);
        assert_eq!(game.for_player(0).unwrap().special_charges(), 0);
        let shared = replica::Frame::capture(game.for_player(0).unwrap());
        assert!(shared.mines.is_empty(), "distant enemy mines stay hidden");
        for player in [1, 2] {
            let frame = replica::Frame::for_player(game.for_player(player).unwrap(), &shared);
            assert_eq!(frame.mines.len(), 1, "all teammates see the placed mine");
            assert_eq!(frame.mines[0].team, Team::Red);
        }
        let body = game.world.pieces[game.players[0].id].body;
        game.world.physics.bodies[body].set_position(Pose::new(Vector::new(15.1, 0.0), 0.0), true);
        game.world
            .physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut game.world.physics.colliders);
        let close = replica::Frame::for_player(game.for_player(0).unwrap(), &shared);
        assert_eq!(
            close.mines.len(),
            1,
            "enemy vehicle reveals it inside three meters"
        );
        game.world.physics.bodies[body].set_position(Pose::new(Vector::new(10.0, 0.0), 0.0), true);
        let far = replica::Frame::for_player(game.for_player(0).unwrap(), &close);
        assert!(
            far.mines.is_empty(),
            "leaving detection immediately hides it again"
        );
    }
    #[test]
    fn each_player_deploys_independently_and_must_choose_again_after_death() {
        let mut game = MultiplayerWorld::new(&Level::default(), &[Team::Blue, Team::Red]).unwrap();
        game.require_vehicle_choices();
        let mut inputs = [PlayerInput::default(); 2];
        inputs[0].deploy = Some(VehicleKind::Scout);
        inputs[1].deploy = Some(VehicleKind::Mortar);
        game.step(&inputs);
        inputs.iter_mut().for_each(|i| i.deploy = None);
        game.step(&inputs);
        assert_eq!(
            game.for_player(0).unwrap().player_vehicle(),
            VehicleKind::Scout
        );
        assert_eq!(
            game.for_player(1).unwrap().player_vehicle(),
            VehicleKind::Mortar
        );
        let id = game.players[1].id;
        game.world
            .tanks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .protection = 0.0;
        game.world.damage_tank(id, 1000.0, &mut vec![]);
        for _ in 0..1200 {
            game.step(&inputs);
        }
        assert!(game.for_player(0).unwrap().tank_alive());
        assert!(!game.for_player(1).unwrap().tank_alive());
        inputs[1].deploy = Some(VehicleKind::Tank);
        game.step(&inputs);
        inputs[1].deploy = None;
        game.step(&inputs);
        assert_eq!(
            game.for_player(1).unwrap().player_vehicle(),
            VehicleKind::Tank
        );
        assert!(game.for_player(1).unwrap().tank_alive());
    }

    #[test]
    fn bridge_repair_takes_fifteen_seconds_with_multiple_players() {
        let mut level = Level::default();
        level.items.push(LevelItem::Bridge {
            min: [-3.0, -3.0],
            max: [3.0, 3.0],
            vertical: false,
            flipped: false,
        });
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        let object = game.world.bodies().find(|b| b.bridge_deck).unwrap().object;
        for bond in &mut game.world.bonds {
            if game.world.pieces[bond.a].object == object
                || game.world.pieces[bond.b].object == object
            {
                bond.active = false;
            }
        }
        game.world.update_bridges(0.0, &mut vec![]);
        let control = game.world.bridges().next().unwrap().controls[0];
        let id = game.players[0].id;
        let pose = (0..64)
            .find_map(|n| {
                let p =
                    control + Vector::from_angle(n as f32 * std::f32::consts::TAU / 64.0) * 0.98;
                let pose = Pose::new(p, 0.0);
                game.world.tank_pose_clear(id, pose).then_some(pose)
            })
            .unwrap();
        let body = game.world.pieces[id].body;
        game.world.physics.bodies[body].set_position(pose, true);
        for _ in 0..1799 {
            game.step(&[PlayerInput::default(); 2]);
        }
        assert!(!game.world.bridges().next().unwrap().intact);
        assert!(game.world.bridges().next().unwrap().repairs[0].progress > 0.99);
        game.step(&[PlayerInput::default(); 2]);
        for player in 0..2 {
            assert!(
                game.for_player(player)
                    .unwrap()
                    .bridges()
                    .next()
                    .unwrap()
                    .intact
            );
        }
    }
    #[test]
    fn airdrop_is_shared_and_only_one_player_can_collect_it() {
        let mut game = MultiplayerWorld::new(&Level::default(), &[Team::Blue, Team::Red]).unwrap();
        game.configure_airdrops(true, 12);
        game.world.tick = 180 * 120 - 1;
        game.step(&[PlayerInput::default(); 2]);
        let drop = game.world.airdrop().unwrap();
        assert!(
            (drop.remaining - 30.0).abs() < 0.001,
            "timer advances once, not per player"
        );
        for player in 0..2 {
            let world = game.for_player(player).unwrap();
            let frame = replica::Frame::capture(world);
            assert_eq!(frame.airdrop, Some(drop));
            let client = World::from_replica(replica::Map::capture(world), frame);
            assert_eq!(client.airdrop(), Some(drop));
        }
        // Both tanks overlap the same pickup before this tick's physics step.
        for player in &game.players {
            let body = game.world.pieces[player.id].body;
            game.world.physics.bodies[body].set_position(Pose::new(drop.position, 0.0), true);
        }
        game.step(&[PlayerInput::default(); 2]);
        assert_eq!(
            game.players
                .iter()
                .filter(|p| p.weapons.charges == 1)
                .count(),
            1
        );
        for player in 0..2 {
            assert!(
                replica::Frame::capture(game.for_player(player).unwrap())
                    .airdrop
                    .is_none()
            );
        }
        game.world.tick = 180 * 120 + 60;
        game.step(&[PlayerInput::default(); 2]);
        assert!(
            game.world.airdrop().is_none(),
            "a used drop does not respawn"
        );
        game.world.tick = 2 * 180 * 120 - 1;
        game.step(&[PlayerInput::default(); 2]);
        let next = game.world.airdrop().unwrap();
        let remote = game.players[1].id;
        let body = game.world.pieces[remote].body;
        game.world.physics.bodies[body].set_position(Pose::new(next.position, 0.0), true);
        game.step(&[PlayerInput::default(); 2]);
        let remote_view = game.for_player(1).unwrap();
        assert_eq!(remote_view.carried_special(), Some(next.weapon));
        assert_eq!(remote_view.special_charges(), 1);
        assert!(game.for_player(0).unwrap().airdrop().is_none());
    }

    #[test]
    fn shared_visibility_cache_matches_uncached_views_and_expires_before_physics() {
        let mut game = MultiplayerWorld::new(
            &Level::river_valley(),
            &[
                Team::Blue,
                Team::Red,
                Team::Blue,
                Team::Red,
                Team::Blue,
                Team::Red,
            ],
        )
        .unwrap();
        let expected: Vec<_> = (0..6)
            .map(|i| {
                let view = game.for_player(i).unwrap();
                view.player_vision(view.player_aim().angle)
            })
            .collect();
        game.prepare_snapshot();
        for (i, expected) in expected.iter().enumerate() {
            let view = game.for_player(i).unwrap();
            assert!(&view.player_vision(view.player_aim().angle) == expected);
        }
        assert!(game.world.sight_cache.is_some());
        game.step(&[]);
        assert!(game.world.sight_cache.is_none());
    }
    #[test]
    fn remote_death_clears_specials_and_respawns_only_that_tank() {
        let mut game =
            MultiplayerWorld::new(&Level::default(), &[Team::Blue, Team::Red, Team::Blue]).unwrap();
        game.activate(2);
        game.world.special = Some(crate::SpecialWeapon::Plasma);
        game.world.special_charges = 1;
        game.save(2);
        let victim = game.players[2].id;
        game.activate(0);
        game.world.destroy_tank(victim, &mut vec![]);
        game.step(&[]);
        assert!(!game.for_player(2).unwrap().tank_alive());
        assert_eq!(game.for_player(2).unwrap().carried_special(), None);
        assert!(game.for_player(0).unwrap().tank_alive());
        let mut respawns = 0;
        for _ in 0..1200 {
            respawns += game
                .step(&[])
                .iter()
                .filter(|e| matches!(e, Event::TankRespawned { .. }))
                .count();
        }
        assert_eq!(respawns, 1);
        assert!(game.for_player(2).unwrap().tank_alive());
        assert_eq!(game.for_player(2).unwrap().special_charges(), 0);
        assert!(game.for_player(1).unwrap().tank_alive());
    }
    #[test]
    fn six_players_have_separate_controls_reload_and_one_shared_clock() {
        let mut game = MultiplayerWorld::new(
            &Level::default(),
            &[
                Team::Blue,
                Team::Red,
                Team::Blue,
                Team::Red,
                Team::Blue,
                Team::Red,
            ],
        )
        .unwrap();
        let starts: Vec<_> = (0..6)
            .map(|i| game.for_player(i).unwrap().launcher_position())
            .collect();
        let mut inputs: Vec<_> = starts
            .iter()
            .map(|p| PlayerInput {
                aim: *p + Vector::Y * 20.0,
                ..Default::default()
            })
            .collect();
        inputs[2].tracks = [1.0; 2];
        for _ in 0..120 {
            game.step(&inputs);
        }
        assert_eq!(game.world().tick, 120);
        assert!(
            game.for_player(2)
                .unwrap()
                .launcher_position()
                .distance(starts[2])
                > 0.3
        );
        assert!(
            game.for_player(4)
                .unwrap()
                .launcher_position()
                .distance(starts[4])
                < 0.01
        );
        inputs[2].fire = true;
        inputs[4].fire = true;
        let events = game.step(&inputs);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Fired { .. }))
                .count(),
            2
        );
        assert!(game.for_player(2).unwrap().reload_remaining() > 0.8);
        assert_eq!(game.for_player(0).unwrap().reload_remaining(), 0.0);
        inputs[2].fire = false;
        inputs[4].fire = false;
        for _ in 0..120 {
            game.step(&inputs);
        }
        assert_eq!(game.for_player(2).unwrap().reload_remaining(), 0.0);
    }
    #[test]
    fn invalid_inputs_cannot_change_weapon_physics_or_inject_nonfinite_state() {
        let mut game = MultiplayerWorld::new(&Level::default(), &[Team::Blue, Team::Red]).unwrap();
        let start = game.for_player(1).unwrap().launcher_position();
        for _ in 0..30 {
            game.step(&[
                PlayerInput::default(),
                PlayerInput {
                    deploy: None,
                    tracks: [f32::NAN, 50.0],
                    aim: Vector::splat(f32::INFINITY),
                    ammunition: Ammunition::Plasma,
                    fire: true,
                    special: true,
                },
            ]);
        }
        assert_eq!(game.for_player(1).unwrap().launcher_position(), start);
        assert_eq!(game.world().shots, 0);
        game.disconnect(1);
        assert_eq!(game.world().winner(), Some(Team::Blue));
    }
    #[test]
    fn remote_specials_and_pickup_timers_are_independent() {
        let mut level = Level::default();
        level.items.push(LevelItem::Pickup {
            weapon: crate::SpecialWeapon::Plasma,
            position: [18.0, 0.0],
            respawn_time: 10.0,
            charges: 1,
            cooldown: 0.0,
        });
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        let mut inputs = [PlayerInput::default(); 2];
        inputs[1].aim = Vector::new(18.0, 20.0);
        game.step(&inputs);
        assert_eq!(game.for_player(1).unwrap().special_charges(), 1);
        assert_eq!(game.for_player(0).unwrap().special_charges(), 0);
        inputs[1].special = true;
        game.step(&inputs);
        inputs[1].special = false;
        assert!(game.for_player(1).unwrap().plasma_remaining() > 0.9);
        assert_eq!(game.for_player(0).unwrap().plasma_remaining(), 0.0);
        let mut fired = 0;
        for _ in 0..120 {
            fired += game
                .step(&inputs)
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        Event::Fired {
                            ammunition: Ammunition::Plasma,
                            ..
                        }
                    )
                })
                .count();
        }
        assert_eq!(fired, 1);
        assert!((game.world().pickups[0].remaining - (10.0 - 121.0 * TICK_DT)).abs() < 0.001);
    }
}
