use super::*;
use std::collections::BTreeMap;

// Bound both live simulation size and retained stable IDs. Rebuilding the scene
// resets this budget. A source can refine a lineage only once, avoiding a
// recursive avalanche inside one impact or one pressure pulse.
const MAX_CELL_RECORDS: usize = 8192;
const MAX_GENERATIONS: u8 = 8;
/// Maximum area of a cell that the laser can remove (about 10 cm detail).
pub const LASER_TILE_AREA: f32 = 0.0125;

impl World {
    /// One pulse either adds detail at its first contact or removes one leaf.
    /// No load, impulse, crack energy or explosive ignition is synthesized.
    pub(super) fn laser_cut(&mut self, id: usize) -> bool {
        let p = &self.pieces[id];
        if !p.active || !p.destructible() || !matches!(p.shape, Shape::Triangle { .. }) {
            return true; // Arena walls and fixtures shield anything behind them.
        }
        if p.shape.area() > LASER_TILE_AREA {
            if p.refinement >= MAX_GENERATIONS || self.pieces.len() + 4 > MAX_CELL_RECORDS {
                self.refinement_limited += 1;
                return false; // Never erase a coarse cell to bypass a budget.
            }
            let event = self.next_impact();
            let first_child = self.pieces.len();
            self.subdivide_cell(id, event);
            // Reveal the local cutting grid; subdivision itself stays rigid.
            for child in &mut self.pieces[first_child..] {
                child.grain = None;
            }
        } else {
            self.ablated += 1;
            self.ablated_area += p.shape.area();
            self.pieces[id].active = false;
            self.pieces[id].debris_remaining = None; // Count cutting separately from cleanup.
            // Vaporized material carries away its own mass and momentum.
            // Survivors retain their old rigid velocity field on rebuilding.
            for bond in &mut self.bonds {
                if bond.a == id || bond.b == id {
                    bond.active = false;
                }
            }
            self.loads.retain(|l| l.piece != id);
            self.blast_pulses.retain(|p| p.piece != id);
            self.topology_changed = true;
        }
        self.structural_cache.clear();
        self.rebuild_chunks();
        true
    }

    pub(super) fn refine_loaded_cells(&mut self) {
        let mut candidates = BTreeMap::new();
        for l in &self.loads {
            let p = &self.pieces[l.piece];
            if !p.active || !p.destructible() || !matches!(p.shape, Shape::Triangle { .. }) {
                continue;
            }
            let components = self.components_for(l.piece);
            // Deformation frames keep their authored connectivity. Fracturing
            // and adaptive subdivision belong to the destructible behavior.
            if components.plasticity.is_some() {
                continue;
            }
            if !l.apply && !components.contact_refinement {
                continue;
            }
            let resistance = if l.apply {
                components.projectile_resistance
            } else {
                components.contact_resistance
            };
            let area = p.shape.area();
            let stress =
                l.impulse.length() / (TICK_DT / SUBSTEPS as f32) / (area.sqrt() * p.solid.depth);
            if area >= DEBRIS_AREA
                && !p.refinement_sources.contains(&l.event)
                && stress > p.solid.strength.tension * p.solid.cohesion * 0.5 * resistance
                && l.energy
                    > p.solid.toughness
                        * area.sqrt()
                        * p.solid.depth
                        * p.solid.cohesion
                        * resistance
            {
                candidates.entry(l.piece).or_insert(l.event);
            }
        }
        let mut changed = false;
        for (id, event) in candidates.into_iter().take(8) {
            if self.pieces.len() + 4 > MAX_CELL_RECORDS
                || self.pieces[id].refinement >= MAX_GENERATIONS
            {
                self.refinement_limited += 1;
                continue;
            }
            self.subdivide_cell(id, event);
            changed = true;
        }
        if changed {
            self.rebuild_chunks();
        }
    }

    fn subdivide_cell(&mut self, id: usize, event: u64) {
        let Shape::Triangle {
            vertices: [a, b, c],
        } = self.pieces[id].shape
        else {
            return;
        };
        let parent_pose = self.piece_pose(id);
        let parent = &self.pieces[id];
        let solid = parent.solid;
        let material = parent.material;
        let friction = parent.floor_friction;
        let restitution = parent.restitution;
        let soft_bounce = parent.soft_bounce;
        let generation = parent.refinement + 1;
        let mut sources = parent.refinement_sources.clone();
        sources.push(event);
        let origin = parent.origin;
        let grain = parent.grain;
        let object = parent.object;
        let reference_area = parent.reference_area;
        let plastic_strain = parent.plastic_strain;
        let shape_revision = parent.shape_revision;
        let ground_anchor = parent.ground_anchor.map(|p| parent_pose.transform_point(p));
        let debris_remaining = parent.debris_remaining;
        let bridge_deck = parent.bridge_deck;
        let previous = Pose::new(parent.previous_position, parent.previous_angle);
        let old_body = &self.physics.bodies[parent.body];
        let center = old_body.center_of_mass();
        let velocity = old_body.linvel();
        let omega = old_body.angvel();
        let ab = (a + b) * 0.5;
        let bc = (b + c) * 0.5;
        let ca = (c + a) * 0.5;
        let triangles = [[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]];
        let mut children = Vec::new();
        for triangle in triangles {
            let local_center = triangle.iter().copied().sum::<Vector>() / 3.0;
            let child =
                self.add_triangle(triangle.map(|v| parent_pose.transform_point(v)), material);
            let piece = &mut self.pieces[child];
            piece.solid = solid;
            piece.floor_friction = friction;
            piece.restitution = restitution;
            piece.soft_bounce = soft_bounce;
            piece.origin = origin;
            piece.fracture_parent = Some(id);
            piece.grain = grain;
            piece.object = object;
            piece.reference_area = reference_area * 0.25;
            piece.plastic_strain = plastic_strain;
            piece.debris_remaining = debris_remaining;
            piece.bridge_deck = bridge_deck;
            piece.shape_revision = shape_revision;
            piece.refinement = generation;
            piece.refinement_sources = sources.clone();
            piece.previous_position = previous.transform_point(local_center);
            // Child vertices are initially expressed in world axes.
            piece.previous_angle = previous.rotation.angle() - parent_pose.rotation.angle();
            let collider = &mut self.physics.colliders[piece.collider];
            collider.set_density(solid.density * solid.depth);
            if shape_revision > 0 {
                collider.set_mass(piece.mass());
            }
            collider.set_friction(friction);
            collider.set_collision_groups(piece.collision_layer().groups());
            collider.set_restitution(restitution);
            collider.set_active_hooks(contacts::hooks_for(soft_bounce));
            let body = &mut self.physics.bodies[piece.body];
            body.recompute_mass_properties_from_colliders(&self.physics.colliders);
            let r = body.center_of_mass() - center;
            body.set_linvel(velocity + Vector::new(-r.y, r.x) * omega, true);
            body.set_angvel(omega, true);
            children.push(child);
        }
        // Preserve old interfaces, including supports and existing cracks.
        // Splitting an interface divides its width; it does not heal a seam.
        let old: Vec<_> = self
            .bonds
            .iter()
            .enumerate()
            .filter(|(_, b)| b.active && (b.a == id || b.b == id))
            .map(|(i, b)| (i, b.clone()))
            .collect();
        for (index, bond) in old {
            let pose = self.piece_pose(bond.a);
            let point = pose.transform_point(bond.anchor_a);
            let normal = pose.rotation * bond.normal_a;
            let tangent = Vector::new(-normal.y, normal.x);
            let start = point - tangent * bond.width * 0.5;
            let end = point + tangent * bond.width * 0.5;
            self.bonds[index].active = false;
            for &child in &children {
                let Shape::Triangle { vertices } = self.pieces[child].shape else {
                    unreachable!()
                };
                let pose = self.piece_pose(child);
                let vertices = vertices.map(|v| pose.transform_point(v));
                for e in 0..3 {
                    if let Some((p, width)) =
                        overlap(start, end, vertices[e], vertices[(e + 1) % 3])
                    {
                        let (a, b) = if bond.a == id {
                            (child, bond.b)
                        } else {
                            (bond.a, child)
                        };
                        self.connect(a, b, p, normal, width, material);
                        let new = self.bonds.last_mut().unwrap();
                        new.strength = bond.strength;
                        new.depth = bond.depth;
                        new.strength_scale = bond.strength_scale;
                        new.young = bond.young;
                        new.toughness = bond.toughness;
                    }
                }
            }
        }
        // The central child has one interface with each corner child.
        for (corner, (a, b)) in [(ab, ca), (ab, bc), (ca, bc)].into_iter().enumerate() {
            let pa = parent_pose.transform_point(a);
            let pb = parent_pose.transform_point(b);
            let from = self.piece_pose(children[corner]).translation;
            let to = self.piece_pose(children[3]).translation;
            let delta = pb - pa;
            let mut normal = Vector::new(-delta.y, delta.x).normalize();
            if normal.dot(to - from) < 0.0 {
                normal = -normal;
            }
            self.connect(
                children[corner],
                children[3],
                (pa + pb) * 0.5,
                normal,
                delta.length(),
                material,
            );
            let bond = self.bonds.last_mut().unwrap();
            bond.strength = solid.strength;
            bond.depth = solid.depth;
            bond.strength_scale = solid.cohesion;
            bond.young = solid.young;
            bond.toughness = solid.toughness;
        }
        // Every source keeps its original work budget and contact location.
        // Pulses follow their material point after the parent body is replaced.
        let child_poses: Vec<_> = children.iter().map(|&i| self.piece_pose(i)).collect();
        let choose = |point: Vector| {
            children
                .iter()
                .enumerate()
                .min_by(|(ia, _), (ib, _)| {
                    let distance = |index: usize| {
                        let local = parent_pose.inverse_transform_point(point);
                        if contains(triangles[index], local) {
                            0.0
                        } else {
                            child_poses[index].translation.distance_squared(point) + 1.0
                        }
                    };
                    distance(*ia).total_cmp(&distance(*ib))
                })
                .map(|(index, &id)| (index, id))
                .unwrap()
        };
        for load in &mut self.loads {
            if load.piece == id {
                load.piece = choose(load.point).1;
            }
        }
        for pulse in &mut self.blast_pulses {
            if pulse.piece == id {
                let point = parent_pose.transform_point(pulse.local_point);
                let (index, child) = choose(point);
                pulse.piece = child;
                pulse.local_point = child_poses[index].inverse_transform_point(point);
            }
        }
        if let Some(point) = ground_anchor {
            let (index, child) = choose(point);
            self.pieces[child].ground_anchor =
                Some(child_poses[index].inverse_transform_point(point));
        }
        self.pieces[id].active = false;
        // Retiring a parent is a geometry replacement, not debris removal.
        self.pieces[id].debris_remaining = None;
        self.refinements += 1;
    }
}

fn contains([a, b, c]: [Vector; 3], p: Vector) -> bool {
    let sign = (b - a).perp_dot(c - a).signum();
    (b - a).perp_dot(p - a) * sign >= -1e-5
        && (c - b).perp_dot(p - b) * sign >= -1e-5
        && (a - c).perp_dot(p - c) * sign >= -1e-5
}

fn overlap(a: Vector, b: Vector, c: Vector, d: Vector) -> Option<(Vector, f32)> {
    let length = a.distance(b);
    let tangent = (b - a) / length;
    if (c - a).perp_dot(tangent).abs() > 1e-4 || (d - a).perp_dot(tangent).abs() > 1e-4 {
        return None;
    }
    let x = (c - a).dot(tangent);
    let y = (d - a).dot(tangent);
    let lo = x.min(y).max(0.0);
    let hi = x.max(y).min(length);
    (hi - lo > 1e-5).then_some((a + tangent * ((lo + hi) * 0.5), hi - lo))
}
