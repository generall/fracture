//! Rate-dependent permanent rest-shape updates. The elastic graph supplies
//! yielded interface modes; a compatible projection keeps cells welded. This
//! is a rigid/plastic approximation, not a dynamic shell or full J2 FEM model.
use super::*;
use crate::structure::{Interface, Node, Solution};
use std::collections::BTreeMap;

fn perpendicular(v: Vector) -> Vector {
    Vector::new(-v.y, v.x)
}
fn root(parent: &mut [usize], i: usize) -> usize {
    if parent[i] != i {
        parent[i] = root(parent, parent[i]);
    }
    parent[i]
}
fn along(p: Vector, a: Vector, b: Vector) -> Option<f32> {
    let e = b - a;
    let t = (p - a).dot(e) / e.length_squared();
    ((-1e-5..=1.0 + 1e-5).contains(&t) && (p - a - e * t).length_squared() < 1e-9)
        .then_some(t.clamp(0.0, 1.0))
}
fn map_point(p: Vector, old: [Vector; 3], new: [Vector; 3]) -> Vector {
    let a = old[1] - old[0];
    let b = old[2] - old[0];
    let q = p - old[0];
    let det = a.perp_dot(b);
    new[0] + (new[1] - new[0]) * (q.perp_dot(b) / det) + (new[2] - new[0]) * (a.perp_dot(q) / det)
}
fn strain(old: [Vector; 3], new: [Vector; 3]) -> f32 {
    let a = old[1] - old[0];
    let b = old[2] - old[0];
    let det = a.perp_dot(b);
    let u = (new[1] - old[1]) - (new[0] - old[0]);
    let v = (new[2] - old[2]) - (new[0] - old[0]);
    let dx = (u * b.y - v * a.y) / det;
    let dy = (-u * b.x + v * a.x) / det;
    (dx.x * dx.x + dy.y * dy.y + 0.5 * (dx.y + dy.x).powi(2)).sqrt()
}
fn overlaps(a: [Vector; 3], b: [Vector; 3]) -> bool {
    for t in [a, b] {
        for i in 0..3 {
            let axis = perpendicular(t[(i + 1) % 3] - t[i]);
            let bounds = |p: [Vector; 3]| {
                let q = p.map(|p| p.dot(axis));
                (
                    q.into_iter().fold(f32::INFINITY, f32::min),
                    q.into_iter().fold(f32::NEG_INFINITY, f32::max),
                )
            };
            let (lo, hi) = bounds(a);
            let (other_lo, other_hi) = bounds(b);
            if hi.min(other_hi) - lo.max(other_lo) <= 1e-5 * axis.length() {
                return false;
            }
        }
    }
    true
}

struct RimEdge {
    a: usize,
    b: usize,
    length: f32,
    corrections: Vec<(usize, f32)>,
}
struct Mesh {
    ids: Vec<usize>,
    old: Vec<[Vector; 3]>,
    delta: Vec<[Vector; 3]>,
    masses: Vec<f32>,
    // Corner corrections expressed in independent welded-vertex DOFs. This
    // also keeps support and hanging-vertex constraints during rim projection.
    stencils: Vec<Vec<(usize, f32)>>,
    mobility: Vec<f32>,
    rim: Vec<RimEdge>,
    free: bool,
}
impl Mesh {
    fn new(
        world: &World,
        ids: &[usize],
        nodes: &[Node],
        u: &[[f64; 3]],
        edges: &[usize],
        frame: Pose,
    ) -> Option<Self> {
        let mut mesh = Self {
            ids: vec![],
            old: vec![],
            delta: vec![],
            masses: vec![],
            stencils: vec![],
            mobility: vec![],
            rim: vec![],
            free: !nodes.iter().any(|n| n.fixed),
        };
        let mut lookup = BTreeMap::new();
        for (i, &id) in ids.iter().enumerate() {
            let p = &world.pieces[id];
            if !p.destructible() {
                continue;
            }
            let Shape::Triangle { vertices } = p.shape else {
                return None;
            };
            let local = p.local_pose;
            let tri = vertices.map(|v| local.transform_point(v));
            let translation = Vector::new(u[i][0] as f32, u[i][1] as f32);
            let center = Vector::new(nodes[i].position[0] as f32, nodes[i].position[1] as f32);
            lookup.insert(id, mesh.ids.len());
            mesh.ids.push(id);
            mesh.old.push(tri);
            mesh.delta
                .push(tri.map(|v| translation + perpendicular(v - center) * u[i][2] as f32));
            mesh.masses.push(p.mass());
        }
        let n = mesh.ids.len() * 3;
        let mut parent: Vec<_> = (0..n).collect();
        let mut pins = Vec::new();
        let mut hanging = Vec::new();
        let mut covered = vec![[0.0; 3]; mesh.ids.len()];
        for &edge in edges {
            let bond = &world.bonds[edge];
            let face = frame
                .inverse_transform_point(world.piece_pose(bond.a).transform_point(bond.anchor_a));
            for id in [bond.a, bond.b] {
                if let Some(&i) = lookup.get(&id) {
                    for (j, width) in covered[i].iter_mut().enumerate() {
                        if along(face, mesh.old[i][j], mesh.old[i][(j + 1) % 3]).is_some() {
                            *width += bond.width;
                        }
                    }
                }
            }
            match (lookup.get(&bond.a), lookup.get(&bond.b)) {
                (Some(&a), Some(&b)) => {
                    for i in 0..3 {
                        for j in 0..3 {
                            if mesh.old[a][i].distance_squared(mesh.old[b][j]) < 1e-9 {
                                let x = root(&mut parent, 3 * a + i);
                                let y = root(&mut parent, 3 * b + j);
                                parent[x] = y;
                            }
                        }
                    }
                    // Refinement can leave hanging vertices on a larger face.
                    for (a, b) in [(a, b), (b, a)] {
                        for i in 0..3 {
                            for j in 0..3 {
                                if let Some(t) =
                                    along(mesh.old[a][i], mesh.old[b][j], mesh.old[b][(j + 1) % 3])
                                    && t > 1e-4
                                    && t < 1.0 - 1e-4
                                {
                                    hanging.push((3 * a + i, 3 * b + j, 3 * b + (j + 1) % 3, t));
                                }
                            }
                        }
                    }
                }
                (Some(&a), None) | (None, Some(&a)) => {
                    let pose = world.piece_pose(bond.a);
                    let point = frame.inverse_transform_point(pose.transform_point(bond.anchor_a));
                    let normal = frame.rotation.inverse() * pose.rotation * bond.normal_a;
                    let extent = perpendicular(normal) * bond.width * 0.5;
                    for i in 0..3 {
                        if along(mesh.old[a][i], point - extent, point + extent).is_some() {
                            pins.push(3 * a + i);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut weights = vec![0.0; n];
        let mut sums = vec![Vector::ZERO; n];
        let mut fixed = vec![false; n];
        for (i, &id) in mesh.ids.iter().enumerate() {
            if world.pieces[id].ground_anchor.is_some() {
                pins.extend([3 * i, 3 * i + 1, 3 * i + 2]);
            }
        }
        for i in 0..n {
            let r = root(&mut parent, i);
            let w = mesh.masses[i / 3] / 3.0;
            weights[r] += w;
            sums[r] += mesh.delta[i / 3][i % 3] * w;
        }
        for p in pins {
            fixed[root(&mut parent, p)] = true;
        }
        for i in 0..n {
            if weights[i] > 0.0 {
                sums[i] /= weights[i];
            }
            if fixed[i] {
                sums[i] = Vector::ZERO;
            }
        }
        let mut constraints = BTreeMap::new();
        for (p, a, b, t) in hanging {
            let p = root(&mut parent, p);
            let a = root(&mut parent, a);
            let b = root(&mut parent, b);
            if p != a && p != b && !fixed[p] {
                constraints.insert(p, (a, b, t));
            }
        }
        fn resolve(
            i: usize,
            sums: &mut [Vector],
            state: &mut [u8],
            constraints: &BTreeMap<usize, (usize, usize, f32)>,
        ) -> Option<Vector> {
            if state[i] == 1 {
                return None;
            }
            if state[i] == 2 {
                return Some(sums[i]);
            }
            state[i] = 1;
            if let Some(&(a, b, t)) = constraints.get(&i) {
                sums[i] = resolve(a, sums, state, constraints)? * (1.0 - t)
                    + resolve(b, sums, state, constraints)? * t;
            }
            state[i] = 2;
            Some(sums[i])
        }
        let mut state = vec![0; n];
        for i in 0..n {
            let r = root(&mut parent, i);
            mesh.delta[i / 3][i % 3] = resolve(r, &mut sums, &mut state, &constraints)?;
        }
        fn stencil(
            i: usize,
            fixed: &[bool],
            constraints: &BTreeMap<usize, (usize, usize, f32)>,
            cache: &mut [Option<Vec<(usize, f32)>>],
        ) -> Vec<(usize, f32)> {
            if let Some(v) = &cache[i] {
                return v.clone();
            }
            let result = if fixed[i] {
                vec![]
            } else if let Some(&(a, b, t)) = constraints.get(&i) {
                let mut combined = BTreeMap::new();
                for (r, w) in stencil(a, fixed, constraints, cache) {
                    *combined.entry(r).or_insert(0.0) += w * (1.0 - t);
                }
                for (r, w) in stencil(b, fixed, constraints, cache) {
                    *combined.entry(r).or_insert(0.0) += w * t;
                }
                combined.into_iter().collect()
            } else {
                vec![(i, 1.0)]
            };
            cache[i] = Some(result.clone());
            result
        }
        let mut cache = vec![None; n];
        for i in 0..n {
            let r = root(&mut parent, i);
            mesh.stencils
                .push(stencil(r, &fixed, &constraints, &mut cache));
        }
        mesh.mobility = weights
            .iter()
            .zip(&fixed)
            .map(|(&w, &pin)| if pin || w == 0.0 { 0.0 } else { 1.0 / w })
            .collect();
        for (i, faces) in covered.iter().enumerate() {
            for (j, &width) in faces.iter().enumerate() {
                if width < 1e-5 {
                    let a = 3 * i + j;
                    let b = 3 * i + (j + 1) % 3;
                    let mut gradient = BTreeMap::new();
                    for &(r, w) in &mesh.stencils[a] {
                        *gradient.entry(r).or_insert(0.0) -= w;
                    }
                    for &(r, w) in &mesh.stencils[b] {
                        *gradient.entry(r).or_insert(0.0) += w;
                    }
                    let denominator: f32 = gradient
                        .iter()
                        .map(|(&r, &w)| mesh.mobility[r] * w * w)
                        .sum();
                    let corrections = gradient
                        .into_iter()
                        .filter_map(|(r, w)| {
                            (denominator > 0.0 && w != 0.0)
                                .then_some((r, mesh.mobility[r] * w / denominator))
                        })
                        .collect();
                    mesh.rim.push(RimEdge {
                        a,
                        b,
                        length: world.pieces[mesh.ids[i]].reference_edges[j],
                        corrections,
                    });
                }
            }
        }
        if !nodes.iter().any(|n| n.fixed) {
            // The free-system gauge must not teleport/rotate the entire object.
            let mass: f32 = mesh.masses.iter().sum();
            let mut center = Vector::ZERO;
            let mut shift = Vector::ZERO;
            for (i, &m) in mesh.masses.iter().enumerate() {
                center += mesh.old[i].iter().copied().sum::<Vector>() * (m / (3.0 * mass));
                shift += mesh.delta[i].iter().copied().sum::<Vector>() * (m / (3.0 * mass));
            }
            let mut spin = 0.0;
            let mut inertia = 0.0;
            for (i, &m) in mesh.masses.iter().enumerate() {
                for j in 0..3 {
                    let r = mesh.old[i][j] - center;
                    spin += m * r.perp_dot(mesh.delta[i][j] - shift);
                    inertia += m * r.length_squared();
                }
            }
            let omega = spin / inertia.max(1e-12);
            for i in 0..mesh.ids.len() {
                for j in 0..3 {
                    mesh.delta[i][j] -= shift + perpendicular(mesh.old[i][j] - center) * omega;
                }
            }
        }
        Some(mesh)
    }
    fn candidate(&self, scale: f32) -> Option<Vec<[Vector; 3]>> {
        let mut next: Vec<_> = self
            .old
            .iter()
            .zip(&self.delta)
            .map(|(p, d)| std::array::from_fn(|i| p[i] + d[i] * scale))
            .collect();
        let mut correction = vec![Vector::ZERO; self.mobility.len()];
        let position = |i: usize, correction: &[Vector]| {
            next[i / 3][i % 3]
                + self.stencils[i]
                    .iter()
                    .map(|&(r, w)| correction[r] * w)
                    .sum::<Vector>()
        };
        // Project the loaded rest-shape proposal onto an inextensible rim.
        // It can bend and enclose less area, but cannot contract like foam.
        // Corrections happen before work, strain and collision acceptance.
        for _ in 0..96 {
            let mut error = 0.0_f32;
            for rim in &self.rim {
                let edge = position(rim.b, &correction) - position(rim.a, &correction);
                let current = edge.length();
                if current < 1e-8 {
                    return None;
                }
                let c = current - rim.length;
                error = error.max(c.abs() / rim.length);
                let step = edge * (-c / current);
                for &(r, w) in &rim.corrections {
                    correction[r] += step * w;
                }
            }
            if error < 2e-6 {
                break;
            }
        }
        if self.rim.iter().any(|e| {
            (position(e.a, &correction).distance(position(e.b, &correction)) - e.length).abs()
                > e.length * 2e-5
        }) {
            return None;
        }
        for (i, triangle) in next.iter_mut().enumerate() {
            for (j, v) in triangle.iter_mut().enumerate() {
                *v += self.stencils[3 * i + j]
                    .iter()
                    .map(|&(r, w)| correction[r] * w)
                    .sum::<Vector>();
            }
        }
        if self.free {
            let mass: f32 = self.masses.iter().sum();
            let center = |triangles: &[[Vector; 3]]| {
                triangles
                    .iter()
                    .zip(&self.masses)
                    .map(|(v, &m)| v.iter().copied().sum::<Vector>() * (m / (3.0 * mass)))
                    .sum::<Vector>()
            };
            let old_center = center(&self.old);
            let new_center = center(&next);
            let mut cross = 0.0;
            let mut dot = 0.0;
            for (i, &m) in self.masses.iter().enumerate() {
                for (&old, &new) in self.old[i].iter().zip(&next[i]) {
                    let a = old - old_center;
                    let b = new - new_center;
                    cross += m * a.perp_dot(b);
                    dot += m * a.dot(b);
                }
            }
            let rotation = Pose::new(Vector::ZERO, -cross.atan2(dot));
            for tri in &mut next {
                for p in tri {
                    *p = rotation.transform_point(*p - new_center) + old_center;
                }
            }
        }
        Some(next)
    }
    fn valid(
        &self,
        world: &World,
        next: &[[Vector; 3]],
        handle: RigidBodyHandle,
        frame: Pose,
    ) -> bool {
        let mut sorted = Vec::new();
        for (i, &tri) in next.iter().enumerate() {
            let old = self.old[i];
            let det = (tri[1] - tri[0]).perp_dot(tri[2] - tri[0]);
            if !tri.iter().all(|p| p.is_finite())
                || det * (old[1] - old[0]).perp_dot(old[2] - old[0]) <= 0.0
                || det.abs() < world.pieces[self.ids[i]].reference_area * 0.002
            {
                return false;
            }
            let lo = tri
                .into_iter()
                .fold(Vector::splat(f32::INFINITY), Vector::min);
            let hi = tri
                .into_iter()
                .fold(Vector::splat(f32::NEG_INFINITY), Vector::max);
            sorted.push((i, lo, hi));
            let Some(new_shape) = SharedShape::convex_hull(&tri) else {
                return false;
            };
            let old_shape = SharedShape::convex_hull(&old).unwrap();
            let bounds = new_shape.compute_aabb(&frame);
            for (_, collider) in world.physics.colliders.iter() {
                if collider.parent() == Some(handle) {
                    continue;
                }
                let other = collider.compute_aabb();
                if bounds.mins.cmpgt(other.maxs).any() || bounds.maxs.cmplt(other.mins).any() {
                    continue;
                }
                use rapier2d::parry::query::contact;
                let Ok(contact_now) = contact(
                    &frame,
                    &*new_shape,
                    collider.position(),
                    collider.shape(),
                    0.0,
                ) else {
                    return false;
                };
                if let Some(c) = contact_now {
                    let Ok(previous) = contact(
                        &frame,
                        &*old_shape,
                        collider.position(),
                        collider.shape(),
                        0.0,
                    ) else {
                        return false;
                    };
                    let previous = previous.map_or(0.0, |c| c.dist.min(0.0));
                    if c.dist < previous - 0.0002 {
                        return false;
                    }
                }
            }
        }
        sorted.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        for (k, &(i, lo, hi)) in sorted.iter().enumerate() {
            for &(j, other_lo, other_hi) in &sorted[k + 1..] {
                if other_lo.x >= hi.x {
                    break;
                }
                if other_lo.y >= hi.y || other_hi.y <= lo.y {
                    continue;
                }
                if overlaps(next[i], next[j]) {
                    return false;
                }
            }
        }
        true
    }
}

impl World {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn deform_plastically(
        &mut self,
        ids: &[usize],
        nodes: &[Node],
        edges: &[usize],
        interfaces: &[Interface],
        solution: &Solution,
        source: &[usize],
        dt: f32,
    ) -> bool {
        let Some(&first) = source.first() else {
            return false;
        };
        let c = self.components_for(self.loads[first].piece);
        let Some(p) = c.plasticity else {
            return false;
        };
        let budget: f32 = source.iter().map(|&i| self.loads[i].energy).sum();
        if budget <= 1e-8 {
            return false;
        }
        let weapon = source.iter().any(|&i| self.loads[i].apply);
        if !weapon && c.crush.is_some_and(|c| !self.squeezed(source, dt, c.force)) {
            return false;
        }
        let resistance = if weapon { c.projectile_resistance } else { 1.0 };
        let mut yielded = false;
        let offsets: Vec<_> = edges
            .iter()
            .zip(interfaces)
            .zip(&solution.reactions)
            .map(|((&id, e), r)| {
                let bond = &self.bonds[id];
                let strain =
                    (self.pieces[bond.a].plastic_strain + self.pieces[bond.b].plastic_strain) * 0.5;
                let yield_stress =
                    (p.yield_stress * resistance * (1.0 + p.hardening * strain)) as f64;
                let area = e.width * e.depth;
                let stress = ((r.normal / area).powi(2) + 3.0 * (r.shear / area).powi(2)).sqrt()
                    + 6.0 * r.moment.abs() / (area * e.width);
                if stress <= yield_stress || strain >= p.max_strain {
                    return [0.0; 3];
                }
                yielded = true;
                let a = nodes[e.a].position;
                let b = nodes[e.b].position;
                let length = ((b[0] - a[0]) * e.normal[0] + (b[1] - a[1]) * e.normal[1])
                    .abs()
                    .max(e.width * 0.02);
                let d = [
                    r.normal * length / (e.young * area),
                    r.shear * length * 2.0 * (1.0 + e.poisson) / (e.young * area),
                    r.moment * length / (e.young * e.depth * e.width.powi(3) / 12.0),
                ];
                let equivalent =
                    (d[0].powi(2) + 0.5 * d[1].powi(2) + (d[2] * e.width * 0.5).powi(2)).sqrt()
                        / length;
                let increment = (p.flow_rate * dt) as f64 * (stress / yield_stress - 1.0).min(4.0);
                d.map(|v| v * increment / equivalent.max(1e-20))
            })
            .collect();
        if !yielded {
            return false;
        }
        let handle = self.pieces[self.loads[first].piece].body;
        let frame = *self.physics.bodies[handle].position();
        let projected = self
            .structural_cache
            .get_mut(&handle.into_raw_parts())
            .unwrap()
            .project_rest_offsets(nodes, &offsets);
        if !projected.converged {
            return false;
        }
        let Some(mesh) = Mesh::new(self, ids, nodes, &projected.displacements, edges, frame) else {
            return false;
        };
        let mut scale = 1.0_f32;
        for (i, d) in mesh.delta.iter().enumerate() {
            let max = d.iter().map(|p| p.length()).fold(0.0, f32::max);
            scale = scale.min(0.015 / max.max(1e-12));
            let delta = strain(mesh.old[i], std::array::from_fn(|j| mesh.old[i][j] + d[j]));
            scale = scale.min(
                (p.max_strain - self.pieces[mesh.ids[i]].plastic_strain).max(0.0)
                    / delta.max(1e-12),
            );
        }
        let body = &self.physics.bodies[handle];
        let velocity = body.linvel();
        let omega = body.angvel();
        let fixed = body.is_fixed();
        let center = frame.inverse_transform_point(body.center_of_mass());
        let moment = |triangles: &[[Vector; 3]]| -> f64 {
            triangles
                .iter()
                .zip(&mesh.masses)
                .map(|(v, &m)| {
                    let centroid = v.iter().copied().sum::<Vector>() / 3.0;
                    Shape::Triangle {
                        vertices: v.map(|p| p - centroid),
                    }
                    .inertia(m) as f64
                        + m as f64 * centroid.distance_squared(center) as f64
                })
                .sum()
        };
        let inertia = moment(&mesh.old);
        let angular = inertia * omega as f64;
        for _ in 0..16 {
            if scale <= 1e-6 {
                return false;
            }
            let Some(next) = mesh.candidate(scale) else {
                scale *= 0.5;
                continue;
            };
            let increments: Vec<_> = mesh
                .old
                .iter()
                .zip(&next)
                .map(|(&a, &b)| strain(a, b))
                .collect();
            // Rim projection is part of the material update, so its displacement
            // and work must obey the same limits as the initial proposal.
            if mesh
                .old
                .iter()
                .flatten()
                .zip(next.iter().flatten())
                .any(|(a, b)| a.distance(*b) > 0.015001)
                || mesh
                    .ids
                    .iter()
                    .zip(&increments)
                    .any(|(&id, &inc)| self.pieces[id].plastic_strain + inc > p.max_strain + 1e-6)
            {
                scale *= 0.5;
                continue;
            }
            let work: f64 = mesh
                .ids
                .iter()
                .zip(&increments)
                .map(|(&id, &inc)| {
                    let piece = &self.pieces[id];
                    let volume = piece.reference_area * piece.solid.depth;
                    (p.yield_stress
                        * resistance
                        * volume
                        * inc
                        * (1.0 + p.hardening * (piece.plastic_strain + inc * 0.5)))
                        as f64
                })
                .sum();
            let new_inertia = moment(&next);
            let kinetic = if fixed {
                0.0
            } else {
                (0.5 * angular * angular / new_inertia.max(1e-12)
                    - 0.5 * inertia * (omega as f64).powi(2))
                .max(0.0)
            };
            let cost = work + kinetic;
            if cost > budget as f64 {
                scale *= ((budget as f64 / cost) as f32 * 0.95).min(0.5);
                continue;
            }
            if work <= 1e-10 {
                return false;
            }
            if !mesh.valid(self, &next, handle, frame) {
                scale *= 0.5;
                continue;
            }
            self.commit_plastic_shape(&mesh, &next, &increments, edges, frame);
            let body = &mut self.physics.bodies[handle];
            body.recompute_mass_properties_from_colliders(&self.physics.colliders);
            if !fixed {
                body.set_linvel(velocity, true);
                body.set_angvel((angular / new_inertia.max(1e-12)) as f32, true);
            }
            let mut remaining = cost as f32;
            for &i in source {
                let used = self.loads[i].energy.min(remaining);
                self.loads[i].energy -= used;
                remaining -= used;
            }
            self.plastic_work += work;
            self.structural_cache.remove(&handle.into_raw_parts());
            self.update_visible_edges();
            return true;
        }
        false
    }
    fn commit_plastic_shape(
        &mut self,
        mesh: &Mesh,
        next: &[[Vector; 3]],
        increments: &[f32],
        edges: &[usize],
        frame: Pose,
    ) {
        let lookup: BTreeMap<_, _> = mesh
            .ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();
        let map = |id: usize, world: Vector| -> Vector {
            lookup.get(&id).map_or(world, |&i| {
                frame.transform_point(map_point(
                    frame.inverse_transform_point(world),
                    mesh.old[i],
                    next[i],
                ))
            })
        };
        let faces: Vec<_> = edges
            .iter()
            .map(|&id| {
                let b = &self.bonds[id];
                let pose = self.piece_pose(b.a);
                let center = pose.transform_point(b.anchor_a);
                let extent = pose.rotation * perpendicular(b.normal_a) * b.width * 0.5;
                let mapped = if lookup.contains_key(&b.a) { b.a } else { b.b };
                // Repeated affine remapping must not drift a complete bonded
                // face away from its vertices (or expose a spurious crack).
                let endpoint = |point: Vector| {
                    let point = map(mapped, point);
                    [b.a, b.b]
                        .iter()
                        .filter_map(|id| lookup.get(id))
                        .flat_map(|&i| next[i])
                        .map(|v| frame.transform_point(v))
                        .filter(|v| v.distance_squared(point) < 1e-8)
                        .min_by(|a, b| {
                            a.distance_squared(point)
                                .total_cmp(&b.distance_squared(point))
                        })
                        .unwrap_or(point)
                };
                (id, endpoint(center - extent), endpoint(center + extent))
            })
            .collect();
        for l in &mut self.loads {
            l.point = map(l.piece, l.point);
        }
        let pulses: Vec<_> = self
            .blast_pulses
            .iter()
            .map(|b| {
                map(
                    b.piece,
                    self.piece_pose(b.piece).transform_point(b.local_point),
                )
            })
            .collect();
        for (i, &id) in mesh.ids.iter().enumerate() {
            let center = next[i].iter().copied().sum::<Vector>() / 3.0;
            let vertices = next[i].map(|p| p - center);
            let p = &mut self.pieces[id];
            p.shape = Shape::Triangle { vertices };
            p.local_pose = Pose::new(center, 0.0);
            p.plastic_strain += increments[i];
            p.shape_revision += 1;
            if increments[i] > 1e-6
                && let Some(object) = p.object
                && self.objects[object].components.explosive.is_some()
            {
                self.objects[object].ignited = true;
            }
            let pose = frame * p.local_pose;
            p.previous_position = pose.translation;
            p.previous_angle = pose.rotation.angle();
            let collider = &mut self.physics.colliders[p.collider];
            collider.set_shape(SharedShape::convex_hull(&vertices).unwrap());
            collider.set_mass(mesh.masses[i]);
            collider.set_position_wrt_parent(p.local_pose);
            collider.set_position(pose);
        }
        for (id, a, b) in faces {
            let bond = &self.bonds[id];
            let pose = self.piece_pose(bond.a);
            let toward = self.piece_pose(bond.b).translation - pose.translation;
            let mut normal = perpendicular(b - a).normalize();
            if normal.dot(toward) < 0.0 {
                normal = -normal;
            }
            let bond = &mut self.bonds[id];
            bond.anchor_a = pose.inverse_transform_point((a + b) * 0.5);
            bond.normal_a = pose.rotation.inverse() * normal;
            bond.width = a.distance(b);
        }
        let locals: Vec<_> = self
            .blast_pulses
            .iter()
            .zip(pulses)
            .map(|(b, p)| self.piece_pose(b.piece).inverse_transform_point(p))
            .collect();
        for (b, local) in self.blast_pulses.iter_mut().zip(locals) {
            b.local_point = local;
        }
    }
}

#[cfg(test)]
mod tests;
