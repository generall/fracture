//! Breakable wall attachments to adjacent and embedded authored fixed obstacles.
//! Built once from the recipe: contact during play never creates new welds.
use super::*;
use std::collections::BTreeMap;

pub(super) struct Edge {
    piece: usize,
    a: Vector,
    b: Vector,
    outward: Vector,
}

pub(super) fn boundary_edges(
    mesh: &crate::ObjectMesh,
    first_piece: usize,
    origin: Vector,
) -> Vec<Edge> {
    let mut edges = BTreeMap::new();
    for (cell, triangle) in mesh.triangles.iter().enumerate() {
        let center = triangle.iter().map(|&i| mesh.vertices[i]).sum::<Vector>() / 3.0;
        for i in 0..3 {
            let (u, v) = (triangle[i], triangle[(i + 1) % 3]);
            let (a, b) = (mesh.vertices[u], mesh.vertices[v]);
            let direction = (b - a).normalize();
            let mut outward = Vector::new(direction.y, -direction.x);
            if outward.dot((a + b) * 0.5 - center) < 0.0 {
                outward = -outward;
            }
            edges
                .entry((u.min(v), u.max(v)))
                .and_modify(|edge| *edge = None)
                .or_insert(Some(Edge {
                    piece: first_piece + cell,
                    a: a + origin,
                    b: b + origin,
                    outward,
                }));
        }
    }
    edges.into_values().flatten().collect()
}

pub(super) fn connect(world: &mut World, walls: &[Edge], solids: &[Edge]) {
    for wall in walls {
        let tangent = (wall.b - wall.a).normalize();
        let length = wall.a.distance(wall.b);
        for support in solids {
            // Only facing, parallel exterior faces can bond. A corner touch,
            // deep overlap, or an internal triangulation edge is not a support.
            if wall.outward.dot(support.outward) > -0.9999 {
                continue;
            }
            let gap_a = (support.a - wall.a).dot(wall.outward);
            let gap_b = (support.b - wall.a).dot(wall.outward);
            if gap_a.min(gap_b) < -0.001 || gap_a.max(gap_b) > 0.2501 {
                continue;
            }
            let u = (support.a - wall.a).dot(tangent);
            let v = (support.b - wall.a).dot(tangent);
            let lo = u.min(v).max(0.0);
            let hi = u.max(v).min(length);
            if hi - lo < 0.001 {
                continue;
            }
            let point =
                wall.a + tangent * ((lo + hi) * 0.5) + wall.outward * ((gap_a + gap_b) * 0.25);
            attach(
                world,
                support.piece,
                wall.piece,
                point,
                -wall.outward,
                hi - lo,
            );
        }
    }
}

fn attach(
    world: &mut World,
    support: usize,
    wall: usize,
    point: Vector,
    normal: Vector,
    width: f32,
) {
    let piece = &world.pieces[wall];
    let (material, solid) = (piece.material, piece.solid);
    world.connect(support, wall, point, normal, width, material);
    let bond = world.bonds.last_mut().unwrap();
    bond.strength = solid.strength;
    bond.depth = solid.depth;
    bond.young = solid.young;
    bond.toughness = solid.toughness;
    bond.strength_scale = solid.cohesion;
}
