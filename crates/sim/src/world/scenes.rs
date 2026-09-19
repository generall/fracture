//! Built-in reference layouts. Object construction remains in `geometry`.
use super::*;
use std::collections::BTreeMap;

mod terrain;

fn noise(i: usize, salt: usize) -> f32 {
    let mut n = (i as u32)
        .wrapping_mul(747_796_405)
        .wrapping_add(salt as u32);
    n = (n ^ (n >> 16)).wrapping_mul(2_246_822_519);
    (n & 0xffff) as f32 / 65_535.0 - 0.5
}

impl World {
    /// Every shared mesh edge is one structural interface. Cells share a rigid
    /// transform until cracks disconnect them.
    fn mesh(
        &mut self,
        vertices: &[Vector],
        triangles: &[[usize; 3]],
        material: Material,
        anchors: &[(usize, f32)],
    ) {
        let first_piece = self.pieces.len();
        let mut edges: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for &triangle in triangles {
            let id = self.add_triangle(triangle.map(|i| vertices[i]), material);
            if material == Material::Rock {
                self.pieces[id].solid.cohesion = 30.0;
            }
            for edge in 0..3 {
                let a = triangle[edge];
                let b = triangle[(edge + 1) % 3];
                let key = (a.min(b), a.max(b));
                if let Some(other) = edges.remove(&key) {
                    let delta = vertices[b] - vertices[a];
                    let mut normal = Vector::new(-delta.y, delta.x).normalize();
                    let pa = self.physics.bodies[self.pieces[other].body].translation();
                    let pb = self.physics.bodies[self.pieces[id].body].translation();
                    if normal.dot(pb - pa) < 0.0 {
                        normal = -normal;
                    }
                    self.connect(
                        other,
                        id,
                        (vertices[a] + vertices[b]) * 0.5,
                        normal,
                        delta.length(),
                        material,
                    );
                    if material == Material::Rock {
                        // Three coherent grains per rock, separated by weaker
                        // seams. Internal grain bonds can still fail under a
                        // sufficiently severe impact; these are strength limits.
                        let grain = |index: usize| (index / 2).min(2);
                        if grain(other - first_piece) == grain(id - first_piece) {
                            self.bonds.last_mut().unwrap().strength_scale = 30.0;
                        }
                    }
                } else {
                    edges.insert(key, id);
                }
            }
        }
        for ((a, b), id) in edges {
            for &(anchor, y) in anchors {
                if (vertices[a].y - y).abs() < 0.001 && (vertices[b].y - y).abs() < 0.001 {
                    let center = self.physics.bodies[self.pieces[id].body].translation();
                    let base = self.physics.bodies[self.pieces[anchor].body].translation();
                    self.connect(
                        anchor,
                        id,
                        (vertices[a] + vertices[b]) * 0.5,
                        Vector::new(0.0, (center.y - base.y).signum()),
                        vertices[a].distance(vertices[b]),
                        material,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn lattice(
        &mut self,
        origin: Vector,
        cols: usize,
        rows: usize,
        spacing: f32,
        material: Material,
        neck: bool,
        anchors: &[(usize, f32)],
    ) {
        let mut vertices = Vec::new();
        for y in 0..=rows {
            for x in 0..=cols {
                let i = y * (cols + 1) + x;
                let jitter = if x == 0 || x == cols || y == 0 || y == rows {
                    Vector::ZERO
                } else {
                    Vector::new(noise(i, 39), noise(i, 1471)) * spacing * 0.65
                };
                vertices.push(origin + Vector::new(x as f32, y as f32) * spacing + jitter);
            }
        }
        let mut triangles = Vec::new();
        for y in 0..rows {
            for x in 0..cols {
                if neck && y < 4 && !(2..4).contains(&x) {
                    continue;
                }
                let a = y * (cols + 1) + x;
                let b = a + 1;
                let c = b + cols + 1;
                let d = a + cols + 1;
                if (x + y) % 2 == 0 {
                    triangles.extend([[a, b, c], [a, c, d]]);
                } else {
                    triangles.extend([[a, b, d], [b, c, d]]);
                }
            }
        }
        self.mesh(&vertices, &triangles, material, anchors);
    }

    pub(super) fn masonry(&mut self) {
        let mut anchors = Vec::new();
        for (center, edge) in [(-4.25, -4.05), (4.25, 4.05)] {
            let id = self.add_box(
                Vector::new(3.025, center),
                Vector::new(1.775, 0.2),
                Material::Anchor,
                true,
            );
            anchors.push((id, edge));
        }
        self.lattice(
            Vector::new(1.45, -4.05),
            7,
            18,
            0.45,
            Material::Masonry,
            false,
            &anchors,
        );
    }

    pub(super) fn barrel_rubble(&mut self) {
        // Open toward the tank, with a closed back and two retaining sides.
        for (center, half) in [
            (Vector::new(4.7, -1.9), Vector::new(1.9, 0.2)),
            (Vector::new(4.7, 1.9), Vector::new(1.9, 0.2)),
            (Vector::new(6.4, 0.0), Vector::new(0.2, 1.7)),
        ] {
            self.add_box(center, half, Material::Anchor, true);
        }
        let asset = crate::LibraryPreset::Barrel.asset();
        let mesh = asset.mesh().expect("valid built-in barrel");
        for position in [
            Vector::new(2.9, 0.0),
            Vector::new(2.05, -1.5),
            Vector::new(2.05, 1.5),
        ] {
            self.add_object(&asset, &mesh, position);
        }

        // Seeded scatter makes reset repeatable. Place larger chunks first,
        // then fill gaps with chips; bounding circles prevent initial overlap.
        let mut placed: Vec<(Vector, f32)> = Vec::new();
        for i in 0..1200 {
            let radius = if i < 100 {
                0.28 + (noise(i, 719) + 0.5) * 0.25
            } else {
                0.07 + (noise(i, 719) + 0.5) * 0.21
            };
            let center = Vector::new(
                3.6 + radius + (noise(i, 173) + 0.5) * (2.6 - 2.0 * radius),
                (noise(i, 983) * 2.0) * (1.7 - radius),
            );
            if placed
                .iter()
                .any(|&(p, r)| center.distance(p) < radius + r + 0.01)
            {
                continue;
            }
            placed.push((center, radius));
            let sides = 3 + i % 4;
            let rotation = (noise(i, 37) + 0.5) * std::f32::consts::TAU;
            let mut vertices = vec![center];
            for j in 0..sides {
                let angle = rotation + j as f32 * std::f32::consts::TAU / sides as f32;
                let extent = radius * (0.8 + (noise(i * 7 + j, 419) + 0.5) * 0.2);
                vertices.push(center + Vector::from_angle(angle) * extent);
            }
            let triangles: Vec<_> = (0..sides)
                .map(|j| [0, j + 1, (j + 1) % sides + 1])
                .collect();
            self.mesh(&vertices, &triangles, Material::Rock, &[]);
        }
        self.configure_test_tank(4500.0, Vector::new(0.77, 0.0), self.tank_speed_limit)
            .expect("valid crushing fixture");
        self.physics.bodies[self.pieces[self.launcher].body].set_linvel(Vector::ZERO, true);
    }

    pub(super) fn cantilever(&mut self) {
        let anchor = self.add_box(
            Vector::new(3.0, -4.3),
            Vector::new(1.45, 0.25),
            Material::Anchor,
            true,
        );
        self.lattice(
            Vector::new(1.65, -4.05),
            6,
            15,
            0.45,
            Material::Panel,
            true,
            &[(anchor, -4.05)],
        );
    }

    pub(super) fn loose_blocks(&mut self) {
        for y in 0..5 {
            for x in 0..3 {
                let index = y * 3 + x;
                let center = Vector::new(0.8 + x as f32 * 2.0, -3.6 + y as f32 * 1.8);
                let mut vertices = vec![center + Vector::new(0.05, -0.07)];
                for i in 0..7 {
                    let angle = i as f32 * std::f32::consts::TAU / 7.0 + index as f32;
                    let radius = 0.61 + noise(i + index * 7, 1723) * 0.28;
                    vertices.push(center + Vector::new(angle.cos(), angle.sin()) * radius);
                }
                let triangles: Vec<_> = (0..7).map(|i| [0, i + 1, (i + 1) % 7 + 1]).collect();
                self.mesh(&vertices, &triangles, Material::Rock, &[]);
            }
        }
    }
}
