//! Small-strain elastic interface graph, independent of rigid-body integration.
//!
//! Each cell has three virtual DOFs (ux, uy, angle). Interfaces resist the
//! relative displacement at their common face and relative rotation. Their
//! stiffness comes from EA/L, GA/L and EI/L, not a mass-dependent spring
//! frequency. Elastic displacements never change collision/render poses.
//! Deformable objects separately project yielded interface rest offsets through
//! this system, then commit a welded, work-limited permanent shape change.
//! A double-precision sparse LDLᵀ solve gives equilibrated seam forces, with
//! residual correction and a bounded iterative fallback. A failed solve must not be used to initiate cracks.

mod factor;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Node {
    pub position: [f64; 2],
    pub fixed: bool,
    pub force: [f64; 3],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Interface {
    pub a: usize,
    pub b: usize,
    pub point: [f64; 2],
    pub normal: [f64; 2],
    pub width: f64,
    pub depth: f64,
    pub young: f64,
    pub poisson: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Reaction {
    /// Positive = opening/tension. Shear and moment have signed local axes.
    pub normal: f64,
    pub shear: f64,
    pub moment: f64,
    pub energy: f64,
}

pub(crate) struct Solution {
    pub displacements: Vec<[f64; 3]>,
    pub reactions: Vec<Reaction>,
    pub residual: f64,
    pub iterations: usize,
    pub converged: bool,
}

struct Element {
    indices: [usize; 6],
    b: [[f64; 6]; 3],
    k: [f64; 3],
}

impl Element {
    fn new(edge: &Interface, nodes: &[Node]) -> Self {
        let a = nodes[edge.a].position;
        let b = nodes[edge.b].position;
        let ra = [edge.point[0] - a[0], edge.point[1] - a[1]];
        let rb = [edge.point[0] - b[0], edge.point[1] - b[1]];
        let n = edge.normal;
        let t = [-n[1], n[0]];
        let cross = |r: [f64; 2], d: [f64; 2]| r[0] * d[1] - r[1] * d[0];
        let row = |d: [f64; 2]| [-d[0], -d[1], -cross(ra, d), d[0], d[1], cross(rb, d)];
        let length = ((b[0] - a[0]) * n[0] + (b[1] - a[1]) * n[1])
            .abs()
            .max(edge.width * 0.02);
        let area = edge.width * edge.depth;
        let inertia = edge.depth * edge.width.powi(3) / 12.0;
        Self {
            indices: [
                3 * edge.a,
                3 * edge.a + 1,
                3 * edge.a + 2,
                3 * edge.b,
                3 * edge.b + 1,
                3 * edge.b + 2,
            ],
            b: [row(n), row(t), [0.0, 0.0, -1.0, 0.0, 0.0, 1.0]],
            k: [
                edge.young * area / length,
                edge.young / (2.0 * (1.0 + edge.poisson)) * area / length,
                edge.young * inertia / length,
            ],
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

/// Cached sparse stiffness and factorization in the chunk's local frame.
pub(crate) struct System {
    elements: Vec<Element>,
    rows: Vec<Vec<(usize, f64)>>,
    scale: Vec<f64>,
    constrained: Vec<bool>,
    factor: Option<factor::Factor>,
    previous: Vec<f64>,
    previous_force: Vec<f64>,
}
impl System {
    /// Least-squares compatible displacement for prescribed interface rest
    /// offsets, using the same stiffness and support constraints as equilibrium.
    pub fn project_rest_offsets(&mut self, nodes: &[Node], offsets: &[[f64; 3]]) -> Solution {
        let mut projected = nodes.to_vec();
        for n in &mut projected {
            n.force = [0.0; 3];
        }
        for (e, d) in self.elements.iter().zip(offsets) {
            for i in 0..6 {
                let force = (0..3).map(|r| e.b[r][i] * e.k[r] * d[r]).sum::<f64>();
                projected[e.indices[i] / 3].force[e.indices[i] % 3] += force;
            }
        }
        self.solve(&projected)
    }
    pub fn new(nodes: &[Node], interfaces: &[Interface]) -> Self {
        let n = nodes.len() * 3;
        let elements: Vec<_> = interfaces.iter().map(|e| Element::new(e, nodes)).collect();
        let anchored = nodes.iter().any(|n| n.fixed);
        let constrained: Vec<_> = nodes
            .iter()
            .enumerate()
            .flat_map(|(i, n)| [n.fixed || (!anchored && i == 0); 3])
            .collect();
        let mut rows = vec![Vec::new(); n];
        for e in &elements {
            for a in 0..6 {
                let i = e.indices[a];
                if constrained[i] {
                    continue;
                }
                for b in 0..6 {
                    let j = e.indices[b];
                    if constrained[j] {
                        continue;
                    }
                    let value = (0..3).map(|r| e.k[r] * e.b[r][a] * e.b[r][b]).sum::<f64>();
                    rows[i].push((j, value));
                }
            }
        }
        let mut scale = vec![1.0; n];
        for i in 0..n {
            if constrained[i] {
                rows[i] = vec![(i, 1.0)];
                continue;
            }
            rows[i].sort_by_key(|(j, _)| *j);
            let mut merged: Vec<(usize, f64)> = Vec::new();
            for (j, value) in std::mem::take(&mut rows[i]) {
                if let Some((last, sum)) = merged.last_mut()
                    && *last == j
                {
                    *sum += value;
                } else {
                    merged.push((j, value));
                }
            }
            scale[i] = 1.0
                / merged
                    .iter()
                    .find(|(j, _)| *j == i)
                    .map_or(1e-20, |(_, v)| *v)
                    .max(1e-20)
                    .sqrt();
            rows[i] = merged;
        }
        for i in 0..n {
            for (j, value) in &mut rows[i] {
                *value *= scale[i] * scale[*j];
            }
        }
        let factor = factor::Factor::new(&rows);
        Self {
            elements,
            rows,
            scale,
            constrained,
            factor,
            previous: vec![0.0; n],
            previous_force: vec![0.0; n],
        }
    }
    fn multiply(&self, x: &[f64], out: &mut [f64]) {
        for (i, row) in self.rows.iter().enumerate() {
            out[i] = row.iter().map(|&(j, k)| k * x[j]).sum();
        }
    }
    fn precondition(&self, r: &[f64], out: &mut [f64]) {
        if let Some(factor) = &self.factor {
            factor.solve(r, out);
        } else {
            out.copy_from_slice(r);
        }
    }
    pub fn solve(&mut self, nodes: &[Node]) -> Solution {
        let n = self.scale.len();
        let force: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.force)
            .enumerate()
            .map(|(i, f)| {
                if self.constrained[i] {
                    0.0
                } else {
                    f * self.scale[i]
                }
            })
            .collect();
        let norm = dot(&force, &force).sqrt();
        let projection = dot(&force, &self.previous_force)
            / dot(&self.previous_force, &self.previous_force).max(1e-100);
        let mut x: Vec<_> = self.previous.iter().map(|x| x * projection).collect();
        let mut ap = vec![0.0; n];
        self.multiply(&x, &mut ap);
        let mut r: Vec<_> = force.iter().zip(&ap).map(|(f, a)| f - a).collect();
        let mut z = vec![0.0; n];
        self.precondition(&r, &mut z);
        let mut p = z.clone();
        let mut rz = dot(&r, &z);
        let mut iterations = 0;
        let tolerance = (norm * 1e-7).max(1e-13);
        let limit = (n * 2).clamp(32, 1024);
        while dot(&r, &r).sqrt() > tolerance && iterations < limit {
            self.multiply(&p, &mut ap);
            let pap = dot(&p, &ap);
            if !pap.is_finite() || pap <= 0.0 {
                break;
            }
            let alpha = rz / pap;
            for i in 0..n {
                x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }
            iterations += 1;
            if iterations % 64 == 0 {
                self.multiply(&x, &mut ap);
                for i in 0..n {
                    r[i] = force[i] - ap[i];
                }
            }
            self.precondition(&r, &mut z);
            let next = dot(&r, &z);
            if rz.abs() < 1e-300 {
                break;
            }
            let beta = next / rz;
            for i in 0..n {
                p[i] = z[i] + beta * p[i];
            }
            rz = next;
        }
        self.multiply(&x, &mut ap);
        // Check the residual in physical force/moment units as well as scaled
        // solver coordinates; preconditioning must not hide a bad equilibrium.
        let error = force
            .iter()
            .zip(&ap)
            .zip(&self.scale)
            .map(|((f, a), s)| ((f - a) / s).powi(2))
            .sum::<f64>()
            .sqrt();
        let physical_norm = force
            .iter()
            .zip(&self.scale)
            .map(|(f, s)| (f / s).powi(2))
            .sum::<f64>()
            .sqrt();
        let converged =
            error <= (physical_norm * 1e-5).max(1e-6) && x.iter().all(|x| x.is_finite());
        if converged {
            self.previous = x.clone();
            self.previous_force = force;
        }
        let u: Vec<_> = x.iter().zip(&self.scale).map(|(x, s)| x * s).collect();
        let reactions = self
            .elements
            .iter()
            .map(|e| {
                let d =
                    e.b.map(|row| (0..6).map(|i| row[i] * u[e.indices[i]]).sum::<f64>());
                Reaction {
                    normal: e.k[0] * d[0],
                    shear: e.k[1] * d[1],
                    moment: e.k[2] * d[2],
                    energy: (0..3).map(|i| 0.5 * e.k[i] * d[i] * d[i]).sum(),
                }
            })
            .collect();
        Solution {
            displacements: u.as_chunks::<3>().0.to_vec(),
            reactions,
            residual: error / physical_norm.max(1e-7),
            iterations,
            converged,
        }
    }
}
#[cfg(test)]
fn solve(nodes: &[Node], interfaces: &[Interface]) -> Solution {
    System::new(nodes, interfaces).solve(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edge(a: usize, b: usize, x: f64) -> Interface {
        Interface {
            a,
            b,
            point: [x, 0.0],
            normal: [1.0, 0.0],
            width: 1.0,
            depth: 0.25,
            young: 30e9,
            poisson: 0.2,
        }
    }
    #[test]
    fn transfers_axial_force_shear_and_bending_to_a_remote_support() {
        let mut nodes: Vec<_> = (0..8)
            .map(|i| Node {
                position: [i as f64, 0.0],
                fixed: i == 0,
                force: [0.0; 3],
            })
            .collect();
        nodes[7].force = [2000.0, 1000.0, 300.0];
        let edges: Vec<_> = (0..7).map(|i| edge(i, i + 1, i as f64 + 0.5)).collect();
        let s = solve(&nodes, &edges);
        assert!(s.converged, "residual {}", s.residual);
        for (i, r) in s.reactions.iter().enumerate() {
            assert!((r.normal - 2000.0).abs() < 0.01);
            assert!((r.shear - 1000.0).abs() < 0.01);
            assert!(
                (r.moment - (300.0 + (6.5 - i as f64) * 1000.0)).abs() < 0.1,
                "{i}: {r:?}"
            );
        }
    }
    #[test]
    fn modulus_changes_elastic_energy_without_changing_equilibrium_force() {
        let nodes = [
            Node {
                position: [0.0, 0.0],
                fixed: true,
                force: [0.0; 3],
            },
            Node {
                position: [1.0, 0.0],
                fixed: false,
                force: [1000.0, 0.0, 0.0],
            },
        ];
        let mut e = edge(0, 1, 0.5);
        let a = solve(&nodes, &[e]);
        e.young *= 2.0;
        let b = solve(&nodes, &[e]);
        assert!((a.reactions[0].normal - b.reactions[0].normal).abs() < 1e-8);
        assert!((a.reactions[0].energy / b.reactions[0].energy - 2.0).abs() < 1e-8);
    }
    #[test]
    fn unloaded_structure_has_no_stress() {
        let n = [
            Node {
                position: [0.0, 0.0],
                fixed: false,
                force: [0.0; 3],
            },
            Node {
                position: [1.0, 0.0],
                fixed: false,
                force: [0.0; 3],
            },
        ];
        let s = solve(&n, &[edge(0, 1, 0.5)]);
        assert!(s.converged);
        assert_eq!(s.reactions[0].normal, 0.0);
    }
}
