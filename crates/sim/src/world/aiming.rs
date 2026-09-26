//! Fixed-tick turret traverse and a deterministic dispersion envelope.
use super::*;
const MIN_SPREAD: f32 = 2.0_f32.to_radians();
const MAX_SPREAD: f32 = 40.0_f32.to_radians();
const PLAYER_TRAVERSE: f32 = 60.0_f32.to_radians();
pub(super) const ENEMY_TRAVERSE: f32 = 40.0_f32.to_radians();

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct AimView {
    pub angle: f32,
    pub previous_angle: f32,
    /// Half-angle of the possible shot directions, in radians.
    pub spread: f32,
}
#[derive(Clone, Copy)]
pub(super) struct AimState {
    pub angle: f32,
    pub previous_angle: f32,
    pub spread: f32,
    pub active: bool,
    target: Option<Vector>,
    shots: u64,
}
impl AimState {
    pub fn new(angle: f32) -> Self {
        Self {
            angle,
            previous_angle: angle,
            spread: MIN_SPREAD,
            active: false,
            target: None,
            shots: 0,
        }
    }
    pub fn view(self) -> AimView {
        AimView {
            angle: self.angle,
            previous_angle: self.previous_angle,
            spread: self.spread,
        }
    }
    pub fn advance(&mut self, desired: f32, speed: f32, hull_turn: f32, rate: f32) {
        self.active = true;
        self.previous_angle = self.angle;
        let error = (desired - self.angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let turn = error.clamp(-rate * TICK_DT, rate * TICK_DT);
        self.angle = (self.angle + turn).rem_euclid(std::f32::consts::TAU);
        let desired_spread = (MIN_SPREAD
            + 24.0_f32.to_radians() * (speed / 6.5).clamp(0.0, 1.5)
            + 8.0_f32.to_radians() * (turn.abs() / (rate * TICK_DT)).min(1.0)
            + 10.0_f32.to_radians() * hull_turn.abs().min(1.0))
        .min(MAX_SPREAD);
        let tau = if desired_spread > self.spread {
            0.03
        } else {
            0.85
        };
        self.spread += (desired_spread - self.spread) * (1.0 - (-TICK_DT / tau).exp());
    }
    fn shot_offset(self, id: usize) -> f32 {
        // Per-tank sequence: repeatable replays, no random number generator shared with physics.
        let mut x = self.shots.wrapping_add(1).wrapping_mul(0x9e3779b97f4a7c15) ^ id as u64;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^= x >> 31;
        ((x >> 40) as f32 / 16_777_215.0 * 2.0 - 1.0) * self.spread
    }
    fn record_shot(&mut self, ammunition: Ammunition) {
        self.shots = self.shots.wrapping_add(1);
        let kick: f32 = match ammunition {
            Ammunition::HighExplosive | Ammunition::ArmorPiercing => 6.0,
            Ammunition::Rocket => 8.0,
            Ammunition::Laser | Ammunition::Plasma => 0.0,
        };
        self.spread = (self.spread + kick.to_radians()).min(MAX_SPREAD);
    }
}
impl World {
    pub fn player_aim(&self) -> AimView {
        self.replica
            .as_ref()
            .map_or_else(|| self.player_aim.view(), |f| f.aim)
    }
    pub(super) fn update_player_aim(&mut self, commands: &[Command]) {
        for command in commands {
            if let Command::Aim(target) = *command
                && target.is_finite()
            {
                if !self.player_aim.active {
                    self.player_aim =
                        AimState::new(self.piece_pose(self.launcher).rotation.angle());
                }
                self.player_aim.target = Some(target);
                self.player_aim.active = true;
            }
        }
        if !self.tank_is_alive(self.launcher) {
            return;
        }
        let Some(target) = self.player_aim.target else {
            return;
        };
        let body = &self.physics.bodies[self.pieces[self.launcher].body];
        let offset = target - body.translation();
        let desired = if offset.length_squared() > 0.01 {
            offset.to_angle()
        } else {
            self.player_aim.angle
        };
        self.player_aim.advance(
            desired,
            body.linvel().length(),
            body.angvel(),
            PLAYER_TRAVERSE,
        );
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == self.launcher) {
            tank.aim = self.player_aim;
        }
    }
    pub(super) fn advance_enemy_aim(&mut self, id: usize, desired: f32) {
        let body = &self.physics.bodies[self.pieces[id].body];
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
            tank.aim.advance(
                desired,
                body.linvel().length(),
                body.angvel(),
                ENEMY_TRAVERSE,
            );
        }
    }
    pub(super) fn shooting_directions(
        &self,
        id: usize,
        requested: Vector,
        ammo: Ammunition,
    ) -> (Vector, Vector) {
        let state = if id == self.launcher {
            Some(self.player_aim)
        } else {
            self.tanks.iter().find(|t| t.id == id).map(|t| t.aim)
        };
        let Some(state) = state.filter(|s| s.active) else {
            // Scripted material trials without Aim commands retain their exact shot setup.
            return (requested, requested);
        };
        let offset = if ammo.energy_beam() {
            0.0
        } else {
            state.shot_offset(id)
        };
        (
            Vector::from_angle(state.angle),
            Vector::from_angle(state.angle + offset),
        )
    }
    pub(super) fn record_aimed_shot(&mut self, id: usize, ammunition: Ammunition) {
        if id == self.launcher {
            self.player_aim.record_shot(ammunition);
            if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
                tank.aim = self.player_aim;
            }
        } else if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
            tank.aim.record_shot(ammunition);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traverse_is_limited_and_takes_shortest_route() {
        let mut aim = AimState::new(0.0);
        for _ in 0..60 {
            aim.advance(std::f32::consts::PI, 0.0, 0.0, PLAYER_TRAVERSE);
        }
        let traveled = (aim.angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!((traveled.abs() - PLAYER_TRAVERSE * TICK_DT * 60.0).abs() < 0.0001);
        aim = AimState::new(359.0_f32.to_radians());
        for _ in 0..10 {
            aim.advance(1.0_f32.to_radians(), 0.0, 0.0, PLAYER_TRAVERSE);
        }
        assert!((aim.angle - 1.0_f32.to_radians()).abs() < 0.0001);
    }

    #[test]
    fn driving_and_traversing_widen_spread_then_it_recovers_gradually() {
        for (speed, turning) in [(6.5, false), (0.0, true)] {
            let mut aim = AimState::new(0.0);
            for _ in 0..120 {
                aim.advance(
                    if turning { aim.angle + 1.0 } else { aim.angle },
                    speed,
                    0.0,
                    PLAYER_TRAVERSE,
                );
            }
            let wide = aim.spread;
            assert!(wide > 4.0_f32.to_radians());
            aim.advance(aim.angle, 0.0, 0.0, PLAYER_TRAVERSE);
            assert!(aim.spread < wide && aim.spread > wide * 0.95);
            for _ in 0..600 {
                aim.advance(aim.angle, 0.0, 0.0, PLAYER_TRAVERSE);
            }
            assert!(aim.spread < MIN_SPREAD + 0.1_f32.to_radians());
        }
    }

    #[test]
    fn simultaneous_aim_and_fire_use_actual_barrel_and_bounded_spread() {
        let mut world = World::new(Scene::Custom);
        let origin = world.launcher_position();
        let target = origin + Vector::Y * 6.0;
        let events = world.step(&[
            Command::Fire(Shot::new(Ammunition::HighExplosive, target)),
            Command::Aim(target),
        ]);
        let (position, velocity) = events
            .iter()
            .find_map(|e| match e {
                Event::Fired {
                    position, velocity, ..
                } => Some((*position, *velocity)),
                _ => None,
            })
            .unwrap();
        let aim = world.player_aim();
        assert!((aim.angle - PLAYER_TRAVERSE * TICK_DT).abs() < 0.0001);
        assert!(position.distance(origin + Vector::from_angle(aim.angle) * 1.65) < 0.0001);
        assert!((velocity.to_angle() - aim.angle).abs() <= aim.spread);
        assert_eq!(world.player_aim.shots, 1);
        assert!(aim.spread >= MIN_SPREAD + 6.0_f32.to_radians());
        world.fire(Shot::new(Ammunition::HighExplosive, target), &mut vec![]);
        assert_eq!(
            world.player_aim.spread, aim.spread,
            "blocked reload must not add spread"
        );
        assert_eq!(
            world.player_aim.shots, 1,
            "reload must not consume shot randomness"
        );
    }

    #[test]
    fn shot_sequence_is_repeatable_varied_and_inside_cone() {
        let mut aim = AimState::new(0.0);
        aim.spread = MAX_SPREAD;
        let mut low = 0.0_f32;
        let mut high = 0.0_f32;
        for shot in 0..100 {
            aim.shots = shot;
            let offset = aim.shot_offset(4);
            assert_eq!(offset, aim.shot_offset(4));
            assert!(offset.abs() <= MAX_SPREAD);
            low = low.min(offset);
            high = high.max(offset);
        }
        assert!(low < -MAX_SPREAD * 0.8 && high > MAX_SPREAD * 0.8);
    }

    #[test]
    fn laser_leaves_actual_muzzle_and_respects_barrel_cover() {
        let mut world = World::new(Scene::Custom);
        let origin = world.launcher_position();
        let target = origin + Vector::Y * 6.0;
        world.update_player_aim(&[Command::Aim(target)]);
        let barrel = Vector::from_angle(world.player_aim.angle);
        world.fire(Shot::new(Ammunition::Laser, target), &mut vec![]);
        assert!(
            world
                .laser_beam
                .unwrap()
                .start
                .distance(origin + barrel * 1.65)
                < 0.0001
        );
        world.add_box(
            origin + barrel * 1.4,
            Vector::splat(0.1),
            Material::Anchor,
            true,
        );
        world.fire(Shot::new(Ammunition::Laser, target), &mut vec![]);
        let beam = world.laser_beam.unwrap();
        assert!(beam.end.distance(origin) < 1.5);
        assert_eq!(beam.start, beam.end);
    }
}
