use super::*;
use std::collections::BTreeMap;

impl World {
    pub(super) fn add_object(
        &mut self,
        object: &crate::ObjectAsset,
        mesh: &crate::ObjectMesh,
        origin: Vector,
    ) {
        let material = object.surface.material();
        let object_id = self.objects.len();
        self.objects.push(components::ObjectInstance {
            color: object.color,
            components: crate::Components {
                blocks_vision: Some(object.blocks_vision()),
                ..object.components
            },
            ignited: false,
            spent: false,
        });
        let first_piece = self.pieces.len();
        let first_bond = self.bonds.len();
        let vertices: Vec<_> = mesh.vertices.iter().map(|v| *v + origin).collect();
        let mut edges: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        let ground_anchor = object.ground_anchor.then(|| mesh.ground_anchor());
        let grounded_grains = if object.ground_anchor {
            mesh.ground_anchor_grains()
        } else {
            Default::default()
        };
        for (index, &triangle) in mesh.triangles.iter().enumerate() {
            let id = self.add_triangle(triangle.map(|i| vertices[i]), material);
            let piece = &mut self.pieces[id];
            piece.floor_friction = object.friction;
            piece.restitution = object.restitution;
            piece.soft_bounce = object.soft_bounce;
            piece.object = Some(object_id);
            if let Some((cell, point)) = ground_anchor
                && grounded_grains.contains(&mesh.grains[index])
            {
                // The central authored stones form a finite buried base.
                // Spread the attachment across it instead of concentrating the
                // entire reaction on a single triangle at the centroid.
                piece.ground_anchor = Some(if cell == index {
                    point + origin - piece.previous_position
                } else {
                    Vector::ZERO
                });
            }
            piece.solid.density = object.density;
            piece.solid.depth = object.thickness;
            piece.solid.strength = crate::material::Strength {
                tension: object.tension,
                compression: object.compression,
                shear: object.shear,
            };
            piece.solid.cohesion = object.cohesion;
            piece.solid.young = object.young_modulus();
            piece.solid.toughness = object.fracture_energy();
            piece.grain = object
                .layout
                .as_ref()
                .map(|_| first_piece + mesh.grains[index]);
            let body = &mut self.physics.bodies[piece.body];
            // Exact collider area preserves the authored mass.
            for &collider in body.colliders() {
                let collider = &mut self.physics.colliders[collider];
                collider.set_density(object.density * object.thickness);
                collider.set_friction(object.friction);
                if object.components.bending_torque.is_some() {
                    collider.set_friction_combine_rule(CoefficientCombineRule::Min);
                }
                collider.set_restitution(object.restitution);
                collider.set_active_hooks(contacts::hooks_for(object.soft_bounce));
            }
            body.recompute_mass_properties_from_colliders(&self.physics.colliders);
            for edge in 0..3 {
                let (a, b) = (triangle[edge], triangle[(edge + 1) % 3]);
                let key = (a.min(b), a.max(b));
                if let Some(other_index) = edges.remove(&key) {
                    let other = first_piece + other_index;
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
                    if mesh.grains[other_index] == mesh.grains[index] {
                        self.bonds.last_mut().unwrap().strength_scale = object.cohesion;
                    }
                } else {
                    edges.insert(key, index);
                }
            }
        }
        for [a, b] in object.support_segments() {
            let a = a + origin;
            let b = b + origin;
            let tangent = (b - a).normalize();
            let signed_area: f32 = (0..object.outline.len())
                .map(|i| {
                    Vector::from(object.outline[i])
                        .perp_dot(Vector::from(object.outline[(i + 1) % object.outline.len()]))
                })
                .sum();
            let outward = Vector::new(tangent.y, -tangent.x) * signed_area.signum();
            let anchor = self.add_box(
                (a + b) * 0.5 + outward * 0.13,
                Vector::new(a.distance(b) * 0.5, 0.12),
                Material::Anchor,
                true,
            );
            let angle = tangent.y.atan2(tangent.x);
            self.physics.bodies[self.pieces[anchor].body].set_rotation(Rotation::new(angle), true);
            self.pieces[anchor].previous_angle = angle;
            for (&(v, w), &index) in &edges {
                if (vertices[v] - a).perp_dot(tangent).abs() < 0.001
                    && (vertices[w] - a).perp_dot(tangent).abs() < 0.001
                {
                    let u = (vertices[v] - a).dot(tangent);
                    let z = (vertices[w] - a).dot(tangent);
                    let (point, width) = if object.support_spacing.is_some() {
                        let lo = u.min(z).max(0.0);
                        let hi = u.max(z).min(a.distance(b));
                        if hi - lo < 0.0001 {
                            continue;
                        }
                        (a + tangent * ((lo + hi) * 0.5), hi - lo)
                    } else {
                        if u.min(z) < -0.001 || u.max(z) > a.distance(b) + 0.001 {
                            continue;
                        }
                        (
                            (vertices[v] + vertices[w]) * 0.5,
                            vertices[v].distance(vertices[w]),
                        )
                    };
                    self.connect(
                        anchor,
                        first_piece + index,
                        point,
                        -outward,
                        width,
                        material,
                    );
                    if object.support_spacing.is_some() {
                        // Footings hold like the interior of a brick, rather
                        // than letting ordinary mortar release the whole wall.
                        self.bonds.last_mut().unwrap().strength_scale = object.cohesion;
                    }
                }
            }
        }
        for bond in &mut self.bonds[first_bond..] {
            bond.strength = crate::material::Strength {
                tension: object.tension,
                compression: object.compression,
                shear: object.shear,
            };
            bond.depth = object.thickness;
            bond.young = object.young_modulus();
            bond.toughness = object.fracture_energy();
        }
    }

    pub(super) fn add_triangle(&mut self, points: [Vector; 3], material: Material) -> usize {
        let position = (points[0] + points[1] + points[2]) / 3.0;
        // Exact shared boundaries: cells inside one body cannot collide.
        let vertices = points.map(|p| p - position);
        let (body, collider) = self.physics.insert(
            RigidBodyBuilder::dynamic()
                .translation(position)
                .ccd_enabled(true),
            ColliderBuilder::convex_hull(&vertices)
                .expect("non-degenerate mesh triangle")
                .collision_groups(CollisionLayer::Solid.groups())
                .density(material.density() * DEPTH)
                .friction(0.65)
                .restitution(0.04),
        );
        let id = self.pieces.len();
        self.physics.colliders[collider].user_data = (id + 1) as u128;
        self.pieces.push(Piece::new(
            id,
            body,
            collider,
            Shape::Triangle { vertices },
            material,
            position,
        ));
        id
    }
}

impl World {
    pub(super) fn add_box(
        &mut self,
        position: Vector,
        half: Vector,
        material: Material,
        fixed: bool,
    ) -> usize {
        let builder = if fixed {
            RigidBodyBuilder::fixed()
        } else {
            RigidBodyBuilder::dynamic()
        };
        let (body, collider) = self.physics.insert(
            builder.translation(position).ccd_enabled(!fixed),
            ColliderBuilder::cuboid(half.x, half.y)
                .collision_groups(CollisionLayer::Solid.groups())
                .density(material.density() * DEPTH)
                .friction(0.55)
                .restitution(0.08),
        );
        let id = self.pieces.len();
        self.physics.colliders[collider].user_data = (id + 1) as u128;
        self.pieces.push(Piece::new(
            id,
            body,
            collider,
            Shape::Box { half },
            material,
            position,
        ));
        id
    }

    #[cfg(test)]
    pub(super) fn add_ball(
        &mut self,
        position: Vector,
        radius: f32,
        mass: f32,
        material: Material,
        velocity: Vector,
    ) -> usize {
        let (body, collider) = self.physics.insert(
            RigidBodyBuilder::dynamic()
                .translation(position)
                .linvel(velocity)
                .ccd_enabled(true),
            ColliderBuilder::ball(radius)
                .collision_groups(CollisionLayer::Solid.groups())
                .mass(mass)
                .friction(0.4)
                .restitution(0.15),
        );
        let id = self.pieces.len();
        self.physics.colliders[collider].user_data = (id + 1) as u128;
        self.pieces.push(Piece::new(
            id,
            body,
            collider,
            Shape::Ball { radius },
            material,
            position,
        ));
        id
    }

    pub(super) fn connect(
        &mut self,
        a: usize,
        b: usize,
        point: Vector,
        normal: Vector,
        width: f32,
        material: Material,
    ) {
        self.structural_cache.clear();
        let pose = self.piece_pose(a);
        let solid = Solid::new(material);
        self.bonds.push(Bond {
            active: true,
            a,
            b,
            anchor_a: pose.inverse_transform_point(point),
            normal_a: pose.rotation.inverse() * normal,
            width,
            strength: solid.strength,
            depth: solid.depth,
            load: 0.0,
            strength_scale: 1.0,
            young: solid.young,
            toughness: solid.toughness,
        });
    }
}
