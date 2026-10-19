//! Visiting public pickup markers does not reveal their hidden live contents.
use super::*;

impl Controller {
    pub(super) fn armament_goal(&mut self, o: &Observation, objective: Vec2) -> Option<Vec2> {
        if o.vehicle == VehicleKind::Scout || o.special.is_some() {
            self.supply_goal = None;
            return None;
        }
        if self.breach.is_some() {
            return self.supply_goal.map(|(p, _)| p);
        }
        self.visited_supplies
            .retain(|(_, tick)| o.tick.saturating_sub(*tick) < 10800);
        if let Some((point, since)) = self.supply_goal {
            let seen_empty =
                o.visible(point) && !o.supplies.iter().any(|s| s.position.distance(point) < 1.0);
            let seen_mine = o
                .supplies
                .iter()
                .any(|s| s.weapon == SpecialWeapon::Mine && s.position.distance(point) < 1.0);
            if point.distance(o.position) < 0.9
                || seen_empty
                || seen_mine
                || o.tick.saturating_sub(since) > 10800
            {
                self.visited_supplies.push((point, o.tick));
                self.supply_goal = None;
            } else {
                return Some(point);
            }
        }
        let direct = o.position.distance(objective);
        let best = self
            .map
            .supply_sites
            .iter()
            .copied()
            .filter(|p| p.distance(objective) < direct - 15.0)
            .filter(|p| p.distance(o.position) + p.distance(objective) < direct + 30.0)
            .filter(|p| self.prepared_costs[self.map.index(*p)] == 0)
            .filter(|p| {
                !self
                    .visited_supplies
                    .iter()
                    .any(|(old, _)| old.distance(*p) < 1.0)
            })
            .min_by(|a, b| {
                // Arm before crossing the contested middle of the map. Equal
                // total detours can have very different unarmed travel times.
                (a.distance(o.position) + a.distance(objective) * 0.35)
                    .total_cmp(&(b.distance(o.position) + b.distance(objective) * 0.35))
            })?;
        self.supply_goal = Some((best, o.tick));
        Some(best)
    }
}
