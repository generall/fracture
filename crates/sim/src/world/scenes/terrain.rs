//! Terrain placement and the accepted bridge material recipe.
use crate::world::*;
use crate::{TerrainKind, TerrainPatch};

impl World {
    pub(in crate::world) fn terrain_range(&mut self) {
        self.terrain = vec![
            TerrainPatch {
                kind: TerrainKind::Road,
                min: Vector::new(-11.85, -1.5),
                max: Vector::new(11.85, 1.5),
            },
            TerrainPatch {
                kind: TerrainKind::Sand,
                min: Vector::new(-11.5, -6.9),
                max: Vector::new(-4.2, -2.5),
            },
            TerrainPatch {
                kind: TerrainKind::Sand,
                min: Vector::new(4.2, 2.5),
                max: Vector::new(11.5, 6.9),
            },
            TerrainPatch {
                kind: TerrainKind::Water,
                min: Vector::new(-2.3, -7.5),
                max: Vector::new(2.3, 7.5),
            },
        ];
        let mut asset = crate::LibraryPreset::Wall.asset();
        asset.name = "Bridge span".into();
        // Two exposed piers beside the lane. Both belong to the same material
        // mesh as the deck, so a severed pier or full-width crack removes support.
        asset.outline = vec![
            [-3.4, -1.5],
            [3.4, -1.5],
            [3.4, 2.6],
            [2.6, 2.6],
            [2.6, 1.5],
            [-2.6, 1.5],
            [-2.6, 2.6],
            [-3.4, 2.6],
        ];
        asset.pinned_edge = Some(2);
        asset.additional_pinned_edges = vec![6];
        asset.thickness = 0.3;
        // Reinforced booths withstand pushing and low-speed bumps. A forceful
        // direct ram can still sever their seams; weapon resistance stays 1.
        asset.components.contact_resistance = 1000.0;
        asset.color = Some([218, 223, 191]);
        asset.regenerate_grains(crate::GrainMode::Bricks, [0.8, 0.55], 19);
        let mesh = asset.mesh().expect("valid bridge recipe");
        let first = self.pieces.len();
        let object = self.objects.len();
        self.add_object(&asset, &mesh, Vector::ZERO);
        let anchors: Vec<_> = (first..self.pieces.len())
            .filter(|&id| self.pieces[id].material == Material::Anchor)
            .collect();
        for id in first..self.pieces.len() {
            let p = &mut self.pieces[id];
            if p.object == Some(object) {
                // A cell touching the elevated deck belongs to the lower layer.
                // Entirely exposed pier cells remain physical ramming targets.
                let Shape::Triangle { vertices } = p.shape else {
                    continue;
                };
                p.bridge_deck = vertices
                    .iter()
                    .any(|v| v.y + p.previous_position.y < 1.5 - 1e-4);
                self.physics.colliders[p.collider]
                    .set_collision_groups(p.collision_layer().groups());
            }
        }
        self.bridges.push(terrain::Bridge::new(
            object,
            [anchors[0], anchors[1]],
            TerrainPatch {
                kind: TerrainKind::Bridge,
                min: Vector::new(-3.4, -1.5),
                max: Vector::new(3.4, 1.5),
            },
            [Vector::new(3.0, 2.05), Vector::new(-3.0, 2.05)],
        ));
        self.rebuild_terrain_barriers();
    }
}
