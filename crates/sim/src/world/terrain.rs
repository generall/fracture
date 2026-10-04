use super::*;
use crate::{BridgeView, TerrainKind, TerrainPatch};
use std::collections::BTreeSet;
pub(in crate::world) mod repair;

pub(super) struct Bridge {
    object: usize,
    anchors: [usize; 2],
    deck: TerrainPatch,
    controls: [Vector; 2],
    fall: Option<f32>,
    fragments: Vec<BodyView>,
    checked: Option<(usize, usize, usize, usize)>,
    blueprint: repair::Blueprint,
    repairs: [repair::Capture; 2],
}

impl World {
    pub fn terrain(&self) -> &[TerrainPatch] {
        &self.terrain
    }

    pub fn bridges(&self) -> impl Iterator<Item = BridgeView<'_>> {
        self.bridges
            .iter()
            .map(|b| BridgeView {
                deck: b.deck,
                controls: b.controls,
                intact: b.fall.is_none(),
                fall_progress: b.fall.map_or(0.0, |t| (t / 1.4).min(1.0)),
                fragments: &b.fragments,
                repairs: b.repairs.map(|r| r.view()),
            })
            .chain(
                self.replica
                    .iter()
                    .flat_map(|f| f.bridges.iter().map(replica::Bridge::view)),
            )
    }

    pub fn terrain_at(&self, point: Vector) -> TerrainKind {
        if self.bridges().any(|b| b.intact && b.deck.contains(point)) {
            return TerrainKind::Bridge;
        }
        if let Some(mask) = &self.terrain_mask {
            return mask.at(point);
        }
        // Gaps win over driveable terrain; the newest gap determines its appearance.
        if let Some(patch) = self
            .terrain
            .iter()
            .rev()
            .find(|p| p.kind.blocks_tanks() && p.contains(point))
        {
            return patch.kind;
        }
        self.terrain
            .iter()
            .rev()
            .find(|p| p.contains(point))
            .map_or(TerrainKind::Ground, |p| p.kind)
    }

    pub fn tank_alive(&self) -> bool {
        self.replica
            .as_ref()
            .map_or_else(|| self.tank_is_alive(self.launcher), |f| f.alive)
    }

    pub fn tank_fall_progress(&self) -> f32 {
        self.replica.as_ref().map_or_else(
            || self.tank_fall.map_or(0.0, |t| (t / 1.4).min(1.0)),
            |f| f.fall,
        )
    }

    pub(super) fn rebuild_terrain_barriers(&mut self) {
        for handle in self.terrain_barriers.drain(..) {
            self.physics.remove_collider(handle);
        }
        let mut blocked: Vec<_> = self
            .terrain
            .iter()
            .copied()
            .filter(|p| p.kind.blocks_tanks())
            .collect();
        for b in self.bridges.iter().filter(|b| b.fall.is_none()) {
            blocked = blocked
                .into_iter()
                .flat_map(|p| p.subtract(b.deck))
                .collect();
        }
        for p in blocked {
            let center = (p.min + p.max) * 0.5;
            let half = (p.max - p.min) * 0.5;
            // Cliff faces deflect loose scenery as well as tanks. These
            // colliders have no piece ID, so sight and weapon queries skip them.
            let layer = if p.kind == TerrainKind::Cliff {
                CollisionLayer::CliffBarrier
            } else {
                CollisionLayer::TerrainBarrier
            };
            self.terrain_barriers.push(
                self.physics.insert_collider(
                    ColliderBuilder::cuboid(half.x, half.y)
                        .translation(center)
                        .friction(0.0)
                        .restitution(0.0)
                        .collision_groups(layer.groups()),
                    None,
                ),
            );
        }
    }

    fn bridge_supported(&self, b: &Bridge) -> bool {
        let attached = b.anchors.map(|anchor| {
            self.bonds
                .iter()
                .filter(|bond| bond.active)
                .filter_map(|bond| {
                    let id = if bond.a == anchor {
                        bond.b
                    } else if bond.b == anchor {
                        bond.a
                    } else {
                        return None;
                    };
                    let p = &self.pieces[id];
                    (p.active && p.object == Some(b.object)).then_some(p.body.into_raw_parts())
                })
                .collect::<BTreeSet<_>>()
        });
        attached[0].intersection(&attached[1]).next().is_some()
    }

    fn tank_over_open_gap(&self, id: usize) -> bool {
        let body = &self.physics.bodies[self.pieces[id].body];
        self.terrain_barriers.iter().any(|&barrier| {
            let barrier = &self.physics.colliders[barrier];
            body.colliders().iter().any(|&c| {
                let c = &self.physics.colliders[c];
                rapier2d::parry::query::intersection_test(
                    c.position(),
                    c.shape(),
                    barrier.position(),
                    barrier.shape(),
                )
                .unwrap_or(false)
            })
        })
    }

    pub(super) fn update_bridges(&mut self, dt: f32, events: &mut Vec<Event>) {
        if let Some(t) = &mut self.tank_fall {
            *t = (*t + dt).min(1.4);
        }
        let stamp = (self.pieces.len(), self.broken, self.ablated, self.cleared);
        let mut collapsed = false;
        for i in 0..self.bridges.len() {
            if let Some(t) = &mut self.bridges[i].fall {
                *t = (*t + dt).min(1.4);
                continue;
            }
            if self.bridges[i].checked == Some(stamp) {
                continue;
            }
            self.bridges[i].checked = Some(stamp);
            if self.bridge_supported(&self.bridges[i]) {
                continue;
            }
            let object = self.bridges[i].object;
            self.bridges[i].fragments = self
                .bodies()
                .filter(|b| self.pieces[b.id].object == Some(object))
                .collect();
            self.bridges[i].fall = Some(0.0);
            for p in &mut self.pieces {
                if p.object == Some(object) {
                    p.active = false;
                    p.debris_remaining = None;
                }
            }
            for bond in &mut self.bonds {
                if !self.pieces[bond.a].active || !self.pieces[bond.b].active {
                    bond.active = false;
                }
            }
            self.loads.retain(|l| self.pieces[l.piece].active);
            self.blast_pulses.retain(|p| self.pieces[p.piece].active);
            events.push(Event::BridgeCollapsed {
                position: (self.bridges[i].deck.min + self.bridges[i].deck.max) * 0.5,
            });
            collapsed = true;
        }
        if collapsed {
            self.rebuild_chunks();
            self.structural_cache.clear();
            self.rebuild_terrain_barriers();
            // Test the actual track/hull shapes before the newly closed terrain
            // barrier can push an occupant back onto a bank.
            let sinking: Vec<_> = self
                .tanks
                .iter()
                .filter(|t| t.health > 0.0 && self.tank_over_open_gap(t.id))
                .map(|t| t.id)
                .collect();
            for id in sinking {
                self.destroy_tank(id, events);
            }
            if self.tanks.is_empty() && self.tank_alive() && self.tank_over_open_gap(self.launcher)
            {
                self.tank_fall = Some(0.0);
                let body = &mut self.physics.bodies[self.pieces[self.launcher].body];
                body.set_linvel(Vector::ZERO, false);
                body.set_angvel(0.0, false);
                body.set_enabled(false);
                self.laser_beam = None;
                self.plasma_remaining = 0.0;
                self.plasma_beam = None;
                events.push(Event::TankSunk {
                    position: self.launcher_position(),
                });
            }
        }
    }
}

impl Bridge {
    pub(super) fn new(
        object: usize,
        anchors: [usize; 2],
        deck: TerrainPatch,
        controls: [Vector; 2],
        blueprint: repair::Blueprint,
    ) -> Self {
        Self {
            object,
            anchors,
            deck,
            controls,
            fall: None,
            fragments: Vec::new(),
            checked: None,
            blueprint,
            repairs: Default::default(),
        }
    }
}
