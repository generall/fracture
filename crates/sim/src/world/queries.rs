//! Read-only scene, geometry, statistics and weapon views.
use super::*;

impl World {
    /// Immediate parent of a subdivided material cell; chunk changes keep cell IDs.
    pub fn fracture_parent(&self, id: usize) -> Option<usize> {
        self.pieces.get(id).and_then(|piece| piece.fracture_parent)
    }

    pub fn scene(&self) -> Scene {
        self.scene
    }

    pub fn launcher_position(&self) -> Vector {
        self.physics.bodies[self.pieces[self.launcher].body].translation()
    }

    /// Live hull poses for cosmetic tank effects, without scanning scenery.
    pub fn tank_poses(&self) -> impl Iterator<Item = (usize, Vector, f32)> + '_ {
        std::iter::once(self.launcher)
            .chain(
                self.tanks
                    .iter()
                    .map(|t| t.id)
                    .filter(|&id| id != self.launcher),
            )
            .filter(|&id| self.tank_is_alive(id) && self.pieces[id].active)
            .map(|id| {
                let pose = self.piece_pose(id);
                (id, pose.translation, pose.rotation.angle())
            })
    }

    pub fn bodies(&self) -> impl Iterator<Item = BodyView> + '_ {
        self.pieces.iter().enumerate().filter_map(|(id, piece)| {
            if !piece.active {
                return None;
            }
            let body = self.physics.bodies.get(piece.body)?;
            let pose = *body.position() * piece.local_pose;
            Some(BodyView {
                id,
                object: piece.object,
                chunk: piece.body.into_raw_parts().0,
                origin: piece.origin,
                position: pose.translation,
                previous_position: piece.previous_position,
                angle: pose.rotation.angle(),
                previous_angle: piece.previous_angle,
                velocity: body.velocity_at_point(pose.translation),
                shape: piece.shape,
                shape_revision: piece.shape_revision,
                tank_fragment: piece.tank_fragment,
                plastic_strain: piece.plastic_strain,
                edges: piece.edges,
                material: piece.material,
                color: piece.object.and_then(|i| self.objects[i].color),
                load: piece.load,
                sleeping: body.is_sleeping(),
                fixed: body.is_fixed(),
                opacity: if piece.material == Material::Launcher
                    && !self.tank_is_alive(id)
                    && !self.tanks.is_empty()
                {
                    0.0
                } else if id == self.launcher {
                    1.0 - self.tank_fall_progress()
                } else {
                    piece.debris_remaining.unwrap_or(1.0).clamp(0.0, 1.0)
                },
            })
        })
    }

    pub fn bonds(&self) -> impl Iterator<Item = BondView> + '_ {
        self.bonds.iter().filter(|bond| bond.active).map(|bond| {
            let a = self.piece_pose(bond.a);
            let b = self.piece_pose(bond.b);
            BondView {
                start: a.translation,
                end: b.translation,
                position: a.transform_point(bond.anchor_a),
                normal: a.rotation * bond.normal_a,
                width: bond.width,
                load: bond.load,
            }
        })
    }

    pub fn stats(&self) -> Stats {
        let mut stats = Stats {
            ablated: self.ablated,
            ablated_area: self.ablated_area,
            plastic_work: self.plastic_work,
            plastic_strain: self
                .pieces
                .iter()
                .filter(|p| p.active)
                .map(|p| p.plastic_strain)
                .fold(0.0, f32::max),
            ticks: self.tick,
            shots: self.shots,
            shells: self.shells.len(),
            explosions: self.explosions,
            cleared: self.cleared,
            cleared_area: self
                .pieces
                .iter()
                .filter(|p| !p.active && p.debris_remaining.is_some())
                .map(|p| p.shape.area())
                .sum(),
            peak_load: self.peak_load,
            cells: self
                .pieces
                .iter()
                .filter(|p| p.active && p.destructible())
                .count(),
            chunks: self.chunk_members().len(),
            refinements: self.refinements,
            refinement_limited: self.refinement_limited,
            fracture_work: self.fracture_work,
            structural_residual: self.structural_residual,
            elastic_energy: self.elastic_energy,
            structural_iterations: self.structural_iterations,
            unconverged: self.unconverged,
            ..Default::default()
        };
        for (_, body) in self.physics.bodies.iter() {
            if body.is_dynamic() {
                stats.bodies += 1;
                stats.awake += usize::from(!body.is_sleeping());
                stats.kinetic_energy += body.kinetic_energy();
            }
        }
        stats.bonds = self.bonds.iter().filter(|bond| bond.active).count();
        stats.broken = self.broken;
        stats.kinetic_energy += self
            .shells
            .iter()
            .map(|s| 0.5 * s.mass * s.velocity.length_squared())
            .sum::<f32>();
        stats
    }

    pub fn shells(&self) -> impl Iterator<Item = &Shell> {
        self.shells.iter()
    }

    pub fn reload_remaining(&self) -> f32 {
        self.reload
    }

    pub fn last_ammunition(&self) -> Ammunition {
        self.last_ammunition
    }

    pub fn laser_beam(&self) -> Option<LaserBeam> {
        self.laser_beam
    }
}
