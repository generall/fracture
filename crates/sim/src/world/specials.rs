//! Collectible, single-use weapons and main-gun round changes.
use super::*;
use crate::SpecialWeapon;

#[derive(Clone, Copy, Debug)]
pub struct PickupView {
    pub position: Vector,
    pub weapon: SpecialWeapon,
    pub respawn_time: f32,
    pub remaining: f32,
}
pub(super) type Pickup = PickupView;

impl World {
    pub fn pickups(&self) -> impl Iterator<Item = PickupView> + '_ {
        self.pickups.iter().copied()
    }
    pub fn carried_special(&self) -> Option<SpecialWeapon> {
        self.special
    }
    pub fn laser_remaining(&self) -> f32 {
        self.laser_remaining
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
        if !self.tank_is_alive(self.launcher) || self.laser_remaining > 0.0 {
            return;
        }
        match self.special {
            Some(SpecialWeapon::Laser) => {
                self.special = None;
                self.laser_remaining = 3.0;
                self.laser_pulse_in = 0.0;
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
                    self.special = None;
                }
            }
            None => {}
        }
    }
    pub(super) fn update_specials(&mut self, events: &mut Vec<Event>) {
        for pickup in &mut self.pickups {
            pickup.remaining = (pickup.remaining - TICK_DT).max(0.0);
        }
        if !self.tank_is_alive(self.launcher) {
            self.laser_remaining = 0.0;
            self.special = None;
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
        if self.special.is_none() && self.laser_remaining <= 0.0 {
            let position = self.launcher_position();
            let rotation = self.piece_pose(self.launcher).rotation.inverse();
            if let Some(pickup) = self.pickups.iter_mut().find(|p| {
                let local = rotation * (p.position - position);
                p.remaining <= 0.0 && local.x.abs() <= 1.25 && local.y.abs() <= 1.0
            }) {
                self.special = Some(pickup.weapon);
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
        });
        let saved = level.to_json().unwrap();
        World::from_level(&Level::from_json(&saved).unwrap(), Team::Blue).unwrap()
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
