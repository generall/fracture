//! Collectible weapon charges, independent cooldowns and main-gun round changes.
use super::*;
use crate::SpecialWeapon;

pub(super) const LASER_DURATION: f32 = 3.0;
pub(super) const LASER_PULSES: usize = 30;
pub(super) const PLASMA_CHARGE: f32 = 1.0;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct PickupView {
    pub position: Vector,
    pub weapon: SpecialWeapon,
    pub respawn_time: f32,
    pub charges: u32,
    pub cooldown: f32,
    pub remaining: f32,
}
pub(super) type Pickup = PickupView;

impl World {
    pub fn pickups(&self) -> impl Iterator<Item = PickupView> + '_ {
        self.pickups
            .iter()
            .copied()
            .chain(self.replica.iter().flat_map(|f| f.pickups.iter().copied()))
    }
    pub fn carried_special(&self) -> Option<SpecialWeapon> {
        self.special
    }
    pub fn laser_remaining(&self) -> f32 {
        self.laser_remaining
    }
    pub fn plasma_remaining(&self) -> f32 {
        self.plasma_remaining
    }
    pub fn plasma_beam(&self) -> Option<LaserBeam> {
        self.plasma_beam
    }
    pub fn special_active(&self) -> bool {
        self.laser_remaining > 0.0 || self.plasma_remaining > 0.0
    }
    pub fn special_charges(&self) -> u32 {
        self.special_charges
    }
    pub fn special_cooldown(&self) -> f32 {
        self.special_reload
    }
    fn consume_special_charge(&mut self, duration: f32) {
        self.special_charges -= 1;
        self.special_reload = duration + self.special_interval;
        if self.special_charges == 0 {
            self.special = None;
        }
    }
    pub(super) fn select_main_ammunition(&mut self, ammunition: Ammunition) {
        if ammunition.special() || ammunition == self.primary_ammunition {
            return;
        }
        self.primary_ammunition = ammunition;
        self.reload = ammunition.interval();
        self.last_ammunition = ammunition;
    }
    pub(super) fn activate_special(&mut self, events: &mut Vec<Event>) {
        if !self.tank_is_alive(self.launcher)
            || self.special_active()
            || self.special_reload > 1e-5
            || self.special_charges == 0
        {
            return;
        }
        match self.special {
            Some(SpecialWeapon::Laser) => {
                self.consume_special_charge(LASER_DURATION);
                self.laser_remaining = LASER_DURATION;
                self.laser_pulse_in = 0.0;
            }
            Some(SpecialWeapon::Plasma) => {
                self.consume_special_charge(PLASMA_CHARGE);
                self.plasma_remaining = PLASMA_CHARGE;
                self.plasma_direction = Vector::from_angle(self.player_aim.angle);
            }
            Some(SpecialWeapon::Rocket) => {
                let before = self.shots;
                let reload = self.reload;
                let last = self.last_ammunition;
                self.reload = 0.0;
                let target =
                    self.launcher_position() + Vector::from_angle(self.player_aim.angle) * 30.0;
                self.fire(Shot::new(Ammunition::Rocket, target), events);
                self.reload = reload;
                self.last_ammunition = last;
                if self.shots > before {
                    self.consume_special_charge(0.0);
                }
            }
            None => {}
        }
    }
    #[cfg(test)]
    pub(super) fn update_specials(&mut self, events: &mut Vec<Event>) {
        self.update_specials_for_player(events, true);
    }
    pub(super) fn update_specials_for_player(
        &mut self,
        events: &mut Vec<Event>,
        advance_pickups: bool,
    ) {
        self.special_reload = (self.special_reload - TICK_DT).max(0.0);
        if self.special_reload < TICK_DT * 0.5 {
            self.special_reload = 0.0;
        }
        if advance_pickups {
            for pickup in &mut self.pickups {
                pickup.remaining = (pickup.remaining - TICK_DT).max(0.0);
            }
        }
        if !self.tank_is_alive(self.launcher) {
            self.laser_remaining = 0.0;
            self.plasma_remaining = 0.0;
            self.plasma_beam = None;
            self.special = None;
            self.special_charges = 0;
            self.special_reload = 0.0;
            self.laser_beam = None;
            return;
        }
        if self.laser_remaining > 0.0 {
            self.laser_pulse_in -= TICK_DT;
            let cut = self.laser_pulse_in <= 1e-6;
            if cut {
                self.laser_pulse_in = Ammunition::Laser.interval();
                self.shots += 1;
            }
            let origin = self.launcher_position();
            let barrel = Vector::from_angle(self.player_aim.angle);
            self.reveal_firing_tank(self.launcher);
            self.fire_laser(origin, barrel, barrel, cut, events);
            self.laser_remaining = (self.laser_remaining - TICK_DT).max(0.0);
            if self.laser_remaining < TICK_DT * 0.5 {
                self.laser_remaining = 0.0;
            }
        }
        if self.plasma_remaining > 0.0 {
            self.plasma_remaining = (self.plasma_remaining - TICK_DT).max(0.0);
            if self.plasma_remaining < TICK_DT * 0.5 {
                self.plasma_remaining = 0.0;
                let origin = self.launcher_position();
                let barrel = if self.player_aim.active {
                    Vector::from_angle(self.player_aim.angle)
                } else {
                    self.plasma_direction
                };
                self.fire_plasma(origin, barrel, events);
            }
        }
        if self.special.is_none() && !self.special_active() {
            self.collect_airdrop();
        }
        if self.special.is_none() && !self.special_active() {
            let position = self.launcher_position();
            let rotation = self.piece_pose(self.launcher).rotation.inverse();
            if let Some(pickup) = self.pickups.iter_mut().find(|p| {
                let local = rotation * (p.position - position);
                p.remaining <= 0.0 && local.x.abs() <= 1.25 && local.y.abs() <= 1.0
            }) {
                self.special = Some(pickup.weapon);
                self.special_charges = pickup.charges;
                self.special_interval = pickup.cooldown;
                pickup.remaining = pickup.respawn_time;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelItem, Team};
    fn pickup_level(weapon: SpecialWeapon) -> World {
        let mut level = Level::default();
        level.items.push(LevelItem::Pickup {
            weapon,
            position: [-18.0, 0.0],
            respawn_time: 1.0,
            charges: 1,
            cooldown: 0.0,
        });
        let saved = level.to_json().unwrap();
        World::from_level(&Level::from_json(&saved).unwrap(), Team::Blue).unwrap()
    }
    #[test]
    fn energy_weapons_cut_in_levels_with_more_than_8192_starting_records() {
        for weapon in [SpecialWeapon::Laser, SpecialWeapon::Plasma] {
            for preset in [crate::LibraryPreset::Rock, crate::LibraryPreset::Wall] {
                let mut w = pickup_level(weapon);
                let rock = preset.asset();
                w.add_object(&rock, &rock.mesh().unwrap(), Vector::ZERO);
                // Fixed background geometry fills the old global record budget,
                // but should not spend any of this level's runtime cutting detail.
                while w.pieces.len() < 8200 {
                    let x = (w.pieces.len() % 100) as f32;
                    let y = 50.0 + (w.pieces.len() / 100) as f32;
                    let id = w.add_triangle(
                        [
                            Vector::new(x, y),
                            Vector::new(x + 0.1, y),
                            Vector::new(x, y + 0.1),
                        ],
                        Material::Anchor,
                    );
                    w.physics.bodies[w.pieces[id].body].set_body_type(RigidBodyType::Fixed, false);
                }
                w.rebuild_chunks();
                let original_records = w.pieces.len();
                w.update_specials(&mut vec![]); // Collect the real pickup.
                w.activate_special(&mut vec![]);
                for _ in 0..360 {
                    w.update_specials(&mut vec![]);
                }
                assert!(
                    w.ablated_area > 0.05,
                    "{weapon:?} / {preset:?} did not cut: {} limit hits",
                    w.refinement_limited
                );
                assert!(w.pieces.len() > original_records);
                assert_eq!(w.refinement_limited, 0);
                assert_eq!(w.stats().kinetic_energy, 0.0);
            }
        }
    }

    #[test]
    fn plasma_charges_for_one_second_then_fires_once_and_cools_down() {
        let mut w = pickup_level(SpecialWeapon::Plasma);
        w.pickups[0].charges = 2;
        w.pickups[0].cooldown = 5.0;
        w.step(&[Command::Aim(Vector::new(10.0, 0.0))]);
        w.reload = 0.6;
        w.activate_special(&mut vec![]);
        assert_eq!(w.reload, 0.6);
        assert_eq!(w.special_charges(), 1);
        assert_eq!(w.plasma_remaining(), 1.0);
        let main = Command::Fire(Shot::new(Ammunition::HighExplosive, Vector::ZERO));
        for _ in 0..119 {
            w.step(&[Command::ActivateSpecial, main]);
            assert_eq!(w.shots, 0, "no discharge or main gun while charging");
            assert!(w.plasma_beam().is_none());
            assert_eq!(w.tanks[1].health, 100.0);
        }
        assert!(w.plasma_remaining() > 0.0);
        w.step(&[Command::ActivateSpecial, main]);
        assert_eq!(w.shots, 1);
        assert_eq!(w.plasma_remaining(), 0.0);
        assert!(w.plasma_beam().is_some());
        assert_eq!(w.tanks[1].health, 0.0);
        assert!((w.special_cooldown() - 5.0).abs() < 0.001);
        for _ in 0..30 {
            w.step(&[Command::ActivateSpecial]);
        }
        assert_eq!(w.shots, 1, "visual afterglow cannot deal repeated damage");
        assert!(w.plasma_beam().is_none());
        assert_eq!(w.special_charges(), 1);
    }

    #[test]
    fn plasma_tracks_the_turret_during_charge_without_spread_and_death_cancels_it() {
        let mut w = pickup_level(SpecialWeapon::Plasma);
        w.step(&[Command::Aim(Vector::ZERO)]);
        w.activate_special(&mut vec![]);
        w.player_aim.spread = 0.6;
        let target = w.launcher_position() + Vector::Y * 30.0;
        for _ in 0..120 {
            w.step(&[Command::Aim(target)]);
        }
        let beam = w.plasma_beam().unwrap();
        let barrel = Vector::from_angle(w.player_aim.angle);
        assert!(barrel.y > 0.8, "turret can turn while charging");
        assert!((beam.end - beam.start).normalize().distance(barrel) < 1e-5);
        assert!(beam.start.distance(w.launcher_position() + barrel * 1.65) < 0.001);
        assert_eq!(w.special_charges(), 0);
        assert_eq!(w.carried_special(), None);

        let mut w = pickup_level(SpecialWeapon::Plasma);
        w.step(&[Command::Aim(Vector::ZERO)]);
        w.step(&[Command::ActivateSpecial]);
        w.destroy_tank(w.launcher, &mut vec![]);
        assert_eq!(w.plasma_remaining(), 0.0);
        for _ in 0..130 {
            w.step(&[]);
        }
        assert_eq!(w.shots, 0);
        assert!(w.plasma_beam().is_none());
    }

    #[test]
    fn charging_plasma_does_not_block_enemy_fire() {
        let mut w = pickup_level(SpecialWeapon::Plasma);
        w.step(&[Command::Aim(Vector::ZERO)]);
        w.activate_special(&mut vec![]);
        let enemy = w.tanks[1].id;
        w.fire_from(
            enemy,
            Shot::new(Ammunition::HighExplosive, w.launcher_position()),
            &mut vec![],
        );
        assert_eq!(w.shells.len(), 1);
        assert_eq!(w.shells[0].owner, enemy);
        assert_eq!(w.plasma_remaining(), 1.0);
    }

    #[test]
    fn plasma_matches_full_laser_cutting_budget_without_adding_force() {
        let asset = crate::LibraryPreset::Rock.asset();
        let mut laser = World::from_object(&asset).unwrap();
        let mut plasma = World::from_object(&asset).unwrap();
        let origin = laser.launcher_position();
        for _ in 0..LASER_PULSES {
            laser.fire_laser(origin, Vector::X, Vector::X, true, &mut vec![]);
        }
        plasma.fire_plasma(origin, Vector::X, &mut vec![]);
        assert!(plasma.ablated_area > 0.0);
        assert_eq!(plasma.ablated_area, laser.ablated_area);
        assert_eq!(plasma.ablated, laser.ablated);
        assert_eq!(plasma.refinements, laser.refinements);
        assert_eq!(plasma.stats().kinetic_energy, 0.0);
        assert!(plasma.laser_beam().is_none());
        assert_eq!(plasma.shots, 1);
    }

    #[test]
    fn plasma_delivers_full_laser_tank_damage_and_respects_solid_cover() {
        for covered in [false, true] {
            let mut w = pickup_level(SpecialWeapon::Plasma);
            w.tanks[1].health = 1000.0;
            if covered {
                w.add_box(
                    Vector::new(-5.0, 0.0),
                    Vector::new(0.2, 2.0),
                    Material::Anchor,
                    true,
                );
            }
            w.fire_plasma(w.launcher_position(), Vector::X, &mut vec![]);
            assert_eq!(w.tanks[1].health, if covered { 1000.0 } else { 640.0 });
            if covered {
                assert!(w.plasma_beam().unwrap().end.x < -4.7);
            }
        }
    }

    #[test]
    fn three_rockets_respect_five_second_cooldown_and_preserve_main_reload() {
        let mut w = pickup_level(SpecialWeapon::Rocket);
        w.pickups[0].charges = 3;
        w.pickups[0].cooldown = 5.0;
        w.step(&[]);
        assert_eq!(w.special_charges(), 3);
        let handle = w.pieces[w.launcher].body;
        w.physics.bodies[handle].set_translation(Vector::new(-15.0, 0.0), true);
        w.reload = 0.6;
        w.activate_special(&mut vec![]);
        assert_eq!(w.reload, 0.6);
        assert_eq!(w.special_charges(), 2);
        assert_eq!(w.special_cooldown(), 5.0);
        for _ in 0..590 {
            w.step(&[Command::ActivateSpecial]);
        }
        assert_eq!(w.shots, 1);
        assert_eq!(w.special_charges(), 2);
        for _ in 0..20 {
            w.step(&[]);
        }
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.shots, 2);
        assert_eq!(w.special_charges(), 1);
        for _ in 0..610 {
            w.step(&[]);
        }
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.shots, 3);
        assert_eq!(w.special_charges(), 0);
        assert_eq!(w.carried_special(), None);
    }

    #[test]
    fn laser_charge_is_a_complete_burst_then_cools_down() {
        let mut w = pickup_level(SpecialWeapon::Laser);
        w.pickups[0].charges = 2;
        w.pickups[0].cooldown = 5.0;
        w.step(&[]);
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.special_charges(), 1);
        for _ in 0..360 {
            w.step(&[Command::ActivateSpecial]);
        }
        assert_eq!(w.laser_remaining(), 0.0);
        assert!(w.special_cooldown() > 4.9);
        assert_eq!(w.shots, 30);
        for _ in 0..580 {
            w.step(&[Command::ActivateSpecial]);
        }
        assert_eq!(w.shots, 30);
        for _ in 0..30 {
            w.step(&[]);
        }
        w.step(&[Command::ActivateSpecial]);
        assert!(w.laser_remaining() > 2.9);
        assert_eq!(w.special_charges(), 0);
    }

    #[test]
    fn pickup_charge_settings_roundtrip_and_old_maps_keep_single_use_defaults() {
        let mut level = Level::default();
        level.items.push(LevelItem::Pickup {
            weapon: SpecialWeapon::Rocket,
            position: [0.0, 0.0],
            respawn_time: 20.0,
            charges: 3,
            cooldown: 5.0,
        });
        assert_eq!(level, Level::from_json(&level.to_json().unwrap()).unwrap());
        let mut json = serde_json::to_value(&level).unwrap();
        let pickup = json["items"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()
            .as_object_mut()
            .unwrap();
        pickup.remove("charges");
        pickup.remove("cooldown");
        let legacy = Level::from_json(&json.to_string()).unwrap();
        assert!(matches!(
            legacy.items.last(),
            Some(LevelItem::Pickup {
                charges: 1,
                cooldown: 0.0,
                ..
            })
        ));
    }

    #[test]
    fn pickup_is_required_consumed_and_respawns_without_overwriting_inventory() {
        let mut w = pickup_level(SpecialWeapon::Rocket);
        assert_eq!(w.carried_special(), None);
        w.activate_special(&mut vec![]);
        assert_eq!(w.shots, 0);
        w.step(&[Command::Aim(Vector::ZERO)]);
        assert_eq!(w.carried_special(), Some(SpecialWeapon::Rocket));
        assert!(w.pickups[0].remaining > 0.9);
        for _ in 0..130 {
            w.step(&[]);
        }
        assert_eq!(
            w.pickups[0].remaining, 0.0,
            "full slot leaves pickup available"
        );
        // Drive clear before activation; do not immediately collect the replacement.
        let handle = w.pieces[w.launcher].body;
        w.physics.bodies[handle].set_translation(Vector::new(-15.0, 0.0), true);
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.carried_special(), None);
        assert_eq!(w.shots, 1);
        assert!((w.shells[0].velocity.length() - 12.0).abs() < 0.01);
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.shots, 1);
    }
    #[test]
    fn changing_main_rounds_reloads_and_same_selection_does_not_restart_it() {
        let mut w = World::new(Scene::Custom);
        let target = w.launcher_position() + Vector::X * 12.0;
        let fire = Command::Fire(Shot::new(Ammunition::ArmorPiercing, target));
        w.step(&[
            Command::Aim(target),
            Command::SelectAmmunition(Ammunition::ArmorPiercing),
            fire,
        ]);
        assert_eq!(w.shots, 0);
        assert!((w.reload - 0.9).abs() < 0.001);
        for _ in 0..60 {
            w.step(&[Command::SelectAmmunition(Ammunition::ArmorPiercing), fire]);
        }
        assert_eq!(w.shots, 0);
        for _ in 0..50 {
            w.step(&[fire]);
        }
        assert_eq!(w.shots, 1);
        w.step(&[Command::SelectAmmunition(Ammunition::HighExplosive)]);
        assert!((w.reload - 0.9).abs() < 0.001);
    }
    #[test]
    fn laser_runs_three_seconds_without_input_and_ignores_spread() {
        let mut w = pickup_level(SpecialWeapon::Laser);
        let target = w.launcher_position() + Vector::X * 10.0;
        w.step(&[Command::Aim(target)]);
        w.player_aim.spread = 0.6;
        w.step(&[Command::ActivateSpecial]);
        assert_eq!(w.special, None);
        let beam = w.laser_beam.unwrap();
        assert!((beam.end - beam.start).normalize().y.abs() < 1e-5);
        for _ in 0..358 {
            w.step(&[]);
            assert!(w.laser_beam.is_some());
        }
        assert!(w.laser_remaining > 0.0);
        for _ in 0..5 {
            w.step(&[]);
        }
        assert_eq!(w.laser_remaining, 0.0);
        assert!(w.laser_beam.is_none());
        assert_eq!(w.shots, 30);
    }
    #[test]
    fn rocket_blast_fractures_rock() {
        let mut w = World::from_object(&crate::LibraryPreset::Rock.asset()).unwrap();
        let initial_area: f32 = w
            .pieces
            .iter()
            .filter(|p| p.active && p.destructible())
            .map(|p| p.shape.area())
            .sum();
        w.special = Some(SpecialWeapon::Rocket);
        w.special_charges = 1;
        let target = Vector::new(3.0, 0.0);
        w.step(&[Command::Aim(target), Command::ActivateSpecial]);
        for _ in 0..360 {
            w.step(&[]);
        }
        let stats = w.stats();
        assert_eq!(stats.explosions, 1);
        let largest = w
            .chunk_members()
            .values()
            .map(|ids| ids.iter().map(|&id| w.pieces[id].shape.area()).sum::<f32>())
            .fold(0.0_f32, f32::max);
        assert!(
            largest < initial_area * 0.6,
            "rock must shatter, not just chip: {largest}/{initial_area}"
        );
    }
    #[test]
    fn wide_laser_damages_off_axis_tank_but_cannot_cut_through_solid_cover() {
        for covered in [false, true] {
            let mut w = pickup_level(SpecialWeapon::Laser);
            let enemy = w.tanks[1].id;
            w.physics.bodies[w.pieces[enemy].body].set_translation(Vector::new(0.0, 0.95), true);
            if covered {
                w.add_box(
                    Vector::new(-5.0, 0.0),
                    Vector::new(0.2, 2.0),
                    Material::Anchor,
                    true,
                );
            }
            w.step(&[Command::Aim(Vector::new(10.0, 0.0))]);
            w.step(&[Command::ActivateSpecial]);
            for _ in 0..60 {
                w.step(&[]);
            }
            if covered {
                assert_eq!(w.tanks[1].health, 100.0);
            } else {
                assert!(
                    w.tanks[1].health < 50.0,
                    "wide beam must hit the offset tracks"
                );
            }
        }
    }
}
