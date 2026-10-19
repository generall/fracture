//! Strategic routes use the public starting map, never hidden live units.
use super::*;

impl Controller {
    pub(super) fn flanking_goal(&mut self, o: &Observation, objective: Vec2) -> Option<Vec2> {
        if !self.config.perimeter_routes || self.flank_done || self.patrol_goal.is_some() {
            return None;
        }
        if self.flank_point.is_none() {
            let home = self.map.flags.iter().find(|(t, _)| *t == self.map.team)?.1;
            let axis = (objective - home).normalize_or_zero();
            let desired =
                (home + objective) * 0.5 + axis.perp() * self.map.size.min_element() * 0.4;
            self.flank_point = (-2..=2)
                .flat_map(|y| (-2..=2).map(move |x| desired + Vec2::new(x as f32, y as f32) * 4.0))
                .filter(|p| p.abs().cmplt(self.map.size * 0.5 - Vec2::splat(3.0)).all())
                .filter(|p| {
                    self.memory[self.map.index(*p)] < 2
                        && self.prepared_costs[self.map.index(*p)] == 0
                })
                .filter(|p| self.navigation_clearance(*p, o.vehicle) > 0.0)
                .min_by(|a, b| {
                    a.distance_squared(desired)
                        .total_cmp(&b.distance_squared(desired))
                });
        }
        let point = self.flank_point?;
        // A promoted support already near the objective must not go back to
        // the initial detour. Each fresh life can use the perimeter again.
        if o.position.distance(point) < 5.0 || o.position.distance(objective) < 40.0 {
            self.flank_done = true;
            None
        } else {
            Some(point)
        }
    }
}
