//! Render data, shared by live views and last-observed snapshots.
use sim::{BodyView, TankView, TerrainPatch, Vec2, World, level::Team};

#[derive(Clone)]
pub(crate) struct Bridge {
    pub deck: TerrainPatch,
    pub controls: [Vec2; 2],
    pub intact: bool,
    pub fall_progress: f32,
    pub fragments: Vec<BodyView>,
    pub opacity: f32,
}
impl From<sim::BridgeView<'_>> for Bridge {
    fn from(b: sim::BridgeView<'_>) -> Self {
        Self {
            deck: b.deck,
            controls: b.controls,
            intact: b.intact,
            fall_progress: b.fall_progress,
            fragments: b.fragments.to_vec(),
            opacity: 1.0,
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Scene {
    pub bodies: Vec<BodyView>,
    pub tanks: Vec<TankView>,
    pub flags: Vec<(Team, Vec2, f32)>,
    pub bridges: Vec<Bridge>,
}
impl Scene {
    pub fn capture(world: &World) -> Self {
        Self {
            bodies: world.bodies().collect(),
            tanks: world.tanks().collect(),
            flags: world.flags().map(|(t, p)| (t, p, 1.0)).collect(),
            bridges: world.bridges().map(Bridge::from).collect(),
        }
    }
    pub fn append(&mut self, snapshot: &Self, remembered: bool) {
        self.bodies
            .extend(snapshot.bodies.iter().copied().map(|mut body| {
                if remembered {
                    body.opacity *= 0.32;
                    body.previous_position = body.position;
                    body.previous_angle = body.angle;
                }
                body
            }));
        self.tanks.extend(snapshot.tanks.iter().copied());
        self.flags.extend(
            snapshot
                .flags
                .iter()
                .map(|&(team, p, a)| (team, p, a * if remembered { 0.32 } else { 1.0 })),
        );
        self.bridges
            .extend(snapshot.bridges.iter().cloned().map(|mut bridge| {
                if remembered {
                    bridge.opacity *= 0.32;
                }
                bridge
            }));
    }
}
