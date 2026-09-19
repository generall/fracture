//! Per-object response and one-shot fracture-triggered explosives.
use super::*;

pub(super) struct ObjectInstance {
    pub color: Option<[u8; 3]>,
    pub components: crate::Components,
    pub ignited: bool,
    pub spent: bool,
}
impl World {
    pub(super) fn components_for(&self, piece: usize) -> crate::Components {
        self.pieces[piece]
            .object
            .map_or_else(crate::Components::default, |id| self.objects[id].components)
    }
    /// Opposed loads at separated points distinguish pinching from a one-sided
    /// shove. This is a buckling approximation, not plastic shell deformation.
    pub(super) fn squeezed(&self, source: &[usize], dt: f32, threshold: f32) -> bool {
        let contacts: Vec<_> = source
            .iter()
            .map(|&i| &self.loads[i])
            .filter(|l| !l.apply)
            .collect();
        let sum: f32 = contacts.iter().map(|l| l.impulse.length()).sum();
        for a in &contacts {
            if a.impulse.length_squared() < 1e-12 {
                continue;
            }
            let axis = a.impulse.normalize();
            let mut positive = 0.0;
            let mut negative = 0.0;
            let mut left = Vector::ZERO;
            let mut right = Vector::ZERO;
            for b in &contacts {
                let projection = b.impulse.dot(axis);
                if projection > 0.0 {
                    positive += projection;
                    left += b.point * projection;
                } else {
                    negative -= projection;
                    right -= b.point * projection;
                }
            }
            // Opposed normals must act toward the interior, on different faces.
            if positive.min(negative) / dt >= threshold
                && positive.min(negative) > sum * 0.15
                && (right / negative - left / positive).dot(axis) > 0.12
            {
                return true;
            }
        }
        false
    }
    pub(super) fn fracture_resistance(&self, source: &[usize], dt: f32) -> f32 {
        let Some(&first) = source.first() else {
            return 1.0;
        };
        let c = self.components_for(self.loads[first].piece);
        let weapon: f32 = source
            .iter()
            .filter(|&&i| self.loads[i].apply)
            .map(|&i| self.loads[i].impulse.length())
            .sum();
        let contact: f32 = source
            .iter()
            .filter(|&&i| !self.loads[i].apply)
            .map(|&i| self.loads[i].impulse.length())
            .sum();
        let mut contact_resistance = c.contact_resistance;
        if let Some(crush) = c.crush
            && self.squeezed(source, dt, crush.force)
            && c.plasticity.is_none()
        {
            contact_resistance *= crush.resistance;
        }
        if weapon + contact <= 1e-12 {
            1.0
        } else {
            (weapon * c.projectile_resistance + contact * contact_resistance) / (weapon + contact)
        }
    }
    pub(super) fn fracture_ignition(&mut self, bond: usize) {
        let b = &self.bonds[bond];
        let (Some(a), Some(z)) = (self.pieces[b.a].object, self.pieces[b.b].object) else {
            return;
        };
        // Breaking a support is not rupturing the vessel itself.
        if a == z && self.objects[a].components.explosive.is_some() && !self.objects[a].spent {
            self.objects[a].ignited = true;
        }
    }
    pub(super) fn detonate_objects(&mut self, events: &mut Vec<Event>) {
        for id in 0..self.objects.len() {
            if !self.objects[id].ignited || self.objects[id].spent {
                continue;
            }
            self.objects[id].spent = true;
            let Some(explosive) = self.objects[id].components.explosive else {
                continue;
            };
            let members: Vec<_> = self
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.active && p.object == Some(id))
                .map(|(i, _)| i)
                .collect();
            let mass: f32 = members.iter().map(|&i| self.pieces[i].mass()).sum();
            if mass <= 0.0 {
                continue;
            }
            let position = members
                .iter()
                .map(|&i| self.piece_pose(i).translation * self.pieces[i].mass())
                .sum::<Vector>()
                / mass;
            // Consume the vessel before casting rays, so its own collision
            // geometry cannot shield all neighbors from an internal explosion.
            for i in members {
                self.pieces[i].active = false;
                self.pieces[i].debris_remaining = Some(0.0);
                self.cleared += 1;
            }
            for b in &mut self.bonds {
                if !self.pieces[b.a].active || !self.pieces[b.b].active {
                    b.active = false;
                }
            }
            self.topology_changed = true;
            self.structural_cache.clear();
            self.rebuild_chunks();
            let event = self.next_impact();
            self.blast(position, explosive.energy, explosive.radius, event, events);
        }
    }
    /// A small shared yard, with a rigid backstop for pinning barrels. Initial
    /// profiles are intentionally left for the subsequent calibration pass.
    pub(super) fn object_yard(&mut self) {
        use crate::LibraryPreset as P;
        self.add_box(
            Vector::new(5.7, -3.3),
            Vector::new(0.18, 2.1),
            Material::Anchor,
            true,
        );
        for (preset, position) in [
            (P::Barrel, Vector::new(1.0, -3.3)),
            (P::ExplosiveBarrel, Vector::new(2.0, 0.0)),
            (P::Barrel, Vector::new(3.5, 0.35)),
            (P::Wall, Vector::new(8.0, 0.0)),
            (P::Rock, Vector::new(2.5, 3.9)),
            (P::MetalFence, Vector::new(-1.0, 3.7)),
        ] {
            let asset = preset.asset();
            let mesh = asset.mesh().expect("valid built-in library object");
            self.add_object(&asset, &mesh, position);
        }
    }
}

#[cfg(test)]
mod tests;
