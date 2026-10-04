//! Host-owned, single-use supply drops. The clock advances once per physics tick.
use super::*;
use crate::SpecialWeapon;

const INTERVAL: u64 = 180 * 120;
const LIFETIME: u64 = 30 * 120;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AirdropView {
    pub position: Vector,
    pub weapon: SpecialWeapon,
    pub remaining: f32,
}

#[derive(Clone, Copy)]
struct Drop {
    position: Vector,
    weapon: SpecialWeapon,
    expires: u64,
}

#[derive(Default)]
pub(super) struct Airdrops {
    enabled: bool,
    random: u64,
    active: Option<Drop>,
}
impl Airdrops {
    // A separate deterministic stream: drops must not alter weapon/physics randomness.
    fn random(&mut self) -> u64 {
        self.random = self.random.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = self.random;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        n ^ (n >> 31)
    }
    fn coordinate(&mut self) -> f32 {
        (self.random() >> 40) as f32 / (1_u32 << 24) as f32 * 2.0 - 1.0
    }
}

impl World {
    /// Configure at match creation; sandbox worlds leave this disabled.
    pub fn configure_airdrops(&mut self, enabled: bool, seed: u64) {
        self.airdrops = Airdrops {
            enabled,
            random: seed,
            active: None,
        };
    }

    pub fn airdrop(&self) -> Option<AirdropView> {
        if let Some(frame) = &self.replica {
            return frame.airdrop;
        }
        self.airdrops.active.map(|drop| AirdropView {
            position: drop.position,
            weapon: drop.weapon,
            remaining: drop.expires.saturating_sub(self.tick) as f32 * TICK_DT,
        })
    }

    pub(super) fn advance_airdrops(&mut self) {
        if !self.airdrops.enabled {
            return;
        }
        let now = self.tick + 1;
        if self.airdrops.active.is_some_and(|drop| now >= drop.expires) {
            self.airdrops.active = None;
        }
        if !now.is_multiple_of(INTERVAL) {
            return;
        }
        let half = self.arena_size * 0.5 - Vector::splat(2.0);
        // Bounded search on the current battlefield, including destroyed cover
        // and collapsed bridges. Never put a drop in water or inside a wall.
        for _ in 0..64 {
            let position = Vector::new(
                self.airdrops.coordinate() * half.x,
                self.airdrops.coordinate() * half.y,
            );
            if self.terrain_at(position).blocks_tanks()
                || self.tanks.iter().any(|tank| {
                    self.tank_is_alive(tank.id)
                        && self
                            .piece_pose(tank.id)
                            .translation
                            .distance_squared(position)
                            < 2.5 * 2.5
                })
                || !self.tank_pose_clear(self.launcher, Pose::new(position, 0.0))
            {
                continue;
            }
            // Avoid isolated islands/enclosures: at least one team's spawn
            // must have a route whose endpoint is the actual landing position.
            let reachable = self.tanks.iter().any(|tank| {
                self.tank_route(tank.id, tank.spawn.translation, position)
                    .last()
                    .is_some_and(|end| end.distance_squared(position) < 0.01)
            });
            if !reachable {
                continue;
            }
            let weapon = [
                SpecialWeapon::Laser,
                SpecialWeapon::Rocket,
                SpecialWeapon::Plasma,
            ][(self.airdrops.random() % 3) as usize];
            self.airdrops.active = Some(Drop {
                position,
                weapon,
                expires: now + LIFETIME,
            });
            return;
        }
    }

    pub(super) fn collect_airdrop(&mut self) {
        let Some(drop) = self.airdrops.active else {
            return;
        };
        let local = self
            .piece_pose(self.launcher)
            .inverse_transform_point(drop.position);
        if local.x.abs() <= 1.25 && local.y.abs() <= 1.0 {
            self.special = Some(drop.weapon);
            self.special_charges = 1;
            self.special_interval = 0.0;
            self.airdrops.active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        level::{Level, Team},
        matchplay::MatchConfig,
    };

    fn game() -> World {
        let mut world = MatchConfig::default()
            .create_world(&Level::default())
            .unwrap();
        world.enemy = None;
        world
    }

    #[test]
    fn drops_follow_match_ticks_expire_and_do_not_respawn_early() {
        let mut world = game();
        world.tick = INTERVAL - 2;
        world.step(&[]);
        assert!(world.airdrop().is_none());
        world.step(&[]);
        let first = world.airdrop().expect("first drop at three minutes");
        assert!((first.remaining - 30.0).abs() < 0.001);
        world.tick = INTERVAL + LIFETIME - 2;
        world.step(&[]);
        assert!(world.airdrop().unwrap().remaining > 0.0);
        world.step(&[]);
        assert!(world.airdrop().is_none(), "expires at thirty seconds");
        world.tick = INTERVAL * 2 - 1;
        world.step(&[]);
        let second = world.airdrop().expect("next drop at six minutes");
        assert_ne!(first.position, second.position);
        assert!((second.remaining - 30.0).abs() < 0.001);
    }

    #[test]
    fn setting_disables_drops_and_sandbox_has_none() {
        let mut disabled = MatchConfig {
            airdrops: false,
            ..Default::default()
        }
        .create_world(&Level::default())
        .unwrap();
        let mut sandbox = World::from_level(&Level::default(), Team::Blue).unwrap();
        for world in [&mut disabled, &mut sandbox] {
            world.tick = INTERVAL - 1;
            world.step(&[]);
            assert!(world.airdrop().is_none());
        }
        let legacy = r#"{"team":"blue","time_limit_seconds":600,"opponent":"conquer"}"#;
        let config: MatchConfig = serde_json::from_str(legacy).unwrap();
        assert!(config.airdrops);
        let roundtrip: MatchConfig = serde_json::from_str(
            &serde_json::to_string(&MatchConfig {
                airdrops: false,
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        assert!(!roundtrip.airdrops);
    }

    #[test]
    fn pickup_is_single_use_and_respects_alive_and_occupied_inventory() {
        let mut world = game();
        world.airdrops.active = Some(Drop {
            position: world.launcher_position(),
            weapon: SpecialWeapon::Plasma,
            expires: LIFETIME,
        });
        world.special = Some(SpecialWeapon::Rocket);
        world.special_charges = 2;
        world.update_specials(&mut vec![]);
        assert!(world.airdrop().is_some());
        assert_eq!(world.special_charges, 2);
        let index = world
            .tanks
            .iter()
            .position(|t| t.id == world.launcher)
            .unwrap();
        world.tanks[index].health = 0.0;
        world.update_specials(&mut vec![]);
        assert!(world.airdrop().is_some(), "dead tanks cannot collect");
        assert_eq!(world.special, None);
        world.tanks[index].health = 100.0;
        world.update_specials(&mut vec![]);
        assert_eq!(world.special, Some(SpecialWeapon::Plasma));
        assert_eq!(world.special_charges, 1);
        assert!(world.airdrop().is_none());
        world.special = None;
        world.special_charges = 0;
        world.update_specials(&mut vec![]);
        assert_eq!(
            world.special, None,
            "a collected drop cannot be collected again"
        );
    }

    #[test]
    fn seeded_drops_are_repeatable_and_clear_on_both_game_maps() {
        for json in [
            include_str!("../../../../levels/dota-level.json"),
            include_str!("../../../../levels/classic-tanks-2.json"),
        ] {
            let level = Level::from_json(json).unwrap();
            let mut world = MatchConfig::default().create_world(&level).unwrap();
            let mut positions = vec![];
            let mut weapons = vec![];
            for seed in 0..6 {
                world.configure_airdrops(true, seed);
                world.tick = INTERVAL - 1;
                world.advance_airdrops();
                let drop = world.airdrop().expect("game map has a valid landing site");
                assert!(!world.terrain_at(drop.position).blocks_tanks());
                assert!(world.tanks.iter().all(|tank| {
                    world
                        .piece_pose(tank.id)
                        .translation
                        .distance(drop.position)
                        >= 2.5
                }));
                assert!(world.tank_pose_clear(world.launcher, Pose::new(drop.position, 0.0)));
                assert!(positions.iter().all(|p| *p != drop.position));
                positions.push(drop.position);
                weapons.push(drop.weapon);
                world.configure_airdrops(true, seed);
                world.advance_airdrops();
                assert_eq!(world.airdrop(), Some(drop));
            }
            assert!(weapons.iter().any(|w| *w != weapons[0]));
        }
    }
}
