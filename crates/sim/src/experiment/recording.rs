//! Immutable visual geometry revisions, compact frames and checkpoint validation.
use super::*;
use crate::{Material, Shape, Vec2, World};

/// Fixed-size geometry is shared by every recorded frame of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub id: usize,
    pub vertices: [[f32; 2]; 4],
    pub count: u8,
    pub material: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}
/// Compact pose record; coordinates remain in SI units for synchronized views.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoseRecord(pub usize, pub [f32; 2], pub f32, pub u8, pub f32, pub f32);
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    #[serde(default)]
    pub ablated_area: f32,
    #[serde(default)]
    pub plastic_work: f64,
    #[serde(default)]
    pub plastic_strain: f32,
    pub broken: usize,
    pub chunks: usize,
    pub cells: usize,
    pub cleared_area: f32,
    pub largest_area: f32,
    pub fracture_work: f64,
    pub elastic_energy: f64,
    pub kinetic_energy: f32,
    pub unconverged: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam: Option<([f32; 2], [f32; 2])>,
    pub tick: u32,
    pub poses: Vec<PoseRecord>,
    pub shells: Vec<[f32; 2]>,
    pub flashes: Vec<([f32; 2], f32)>,
    pub metrics: Metrics,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualState {
    pub geometry: Vec<Geometry>,
    pub frame: Frame,
}
impl VisualState {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.geometry.len() > 8192
            || self.frame.poses.len() > 8192
            || self.frame.shells.len() > 32
            || self.frame.flashes.len() > 32
        {
            return Err("Saved view exceeds the geometry limit.".into());
        }
        let ids: std::collections::BTreeSet<_> = self.geometry.iter().map(|g| g.id).collect();
        if ids.len() != self.geometry.len() {
            return Err("Saved geometry contains duplicate IDs.".into());
        }
        for g in &self.geometry {
            if !(3..=4).contains(&g.count)
                || g.material > 5
                || g.vertices
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 30.0)
            {
                return Err("Invalid saved geometry.".into());
            }
        }
        let pose_ids: std::collections::BTreeSet<_> =
            self.frame.poses.iter().map(|p| p.0).collect();
        if pose_ids.len() != self.frame.poses.len() {
            return Err("Saved view contains duplicate poses.".into());
        }
        for p in &self.frame.poses {
            if !ids.contains(&p.0)
                || p.1.iter().any(|v| !v.is_finite() || v.abs() > 100_000.0)
                || !p.2.is_finite()
                || !p.4.is_finite()
                || !(0.0..=1.0).contains(&p.4)
                || !p.5.is_finite()
            {
                return Err("Invalid saved pose.".into());
            }
        }
        if self
            .frame
            .shells
            .iter()
            .flatten()
            .chain(self.frame.flashes.iter().flat_map(|(p, _)| p.iter()))
            .chain(
                self.frame
                    .beam
                    .iter()
                    .flat_map(|(a, b)| a.iter().chain(b.iter())),
            )
            .any(|v| !v.is_finite())
            || self
                .frame
                .flashes
                .iter()
                .any(|(_, r)| !r.is_finite() || !(0.0..=10.0).contains(r))
        {
            return Err("Invalid saved effects.".into());
        }
        let m = &self.frame.metrics;
        if [
            m.ablated_area as f64,
            m.cleared_area as f64,
            m.largest_area as f64,
            m.fracture_work,
            m.elastic_energy,
            m.kinetic_energy as f64,
            m.plastic_work,
            m.plastic_strain as f64,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err("Invalid saved measurements.".into());
        }
        Ok(())
    }
}

pub struct Recording {
    pub geometry: Vec<Option<Geometry>>,
    pub frames: Vec<Frame>,
    current_geometry: std::collections::BTreeMap<usize, (u32, usize)>,
}
impl Recording {
    pub fn state(&self, tick: u32) -> VisualState {
        let frame = self
            .frames
            .iter()
            .rev()
            .find(|f| f.tick <= tick)
            .unwrap_or(&self.frames[0])
            .clone();
        let geometry = frame
            .poses
            .iter()
            .filter_map(|p| self.geometry.get(p.0).and_then(Clone::clone))
            .collect();
        VisualState { geometry, frame }
    }
}

impl Recording {
    pub(super) fn new() -> Self {
        Self {
            geometry: vec![],
            frames: vec![],
            current_geometry: Default::default(),
        }
    }
    pub(super) fn capture(
        &mut self,
        w: &World,
        tick: u32,
        flashes: &[([f32; 2], f32, u32)],
        peak_elastic: f64,
    ) -> usize {
        let record = self;
        let mut recorded_poses = 0;
        let stats = w.stats();
        let mut poses = Vec::new();
        let mut areas = std::collections::BTreeMap::<u32, f32>::new();
        for b in w.bodies() {
            let (vertices, count, edges) = match b.shape {
                Shape::Triangle { vertices } => {
                    let [a, z, c] = vertices;
                    (
                        [a.to_array(), z.to_array(), c.to_array(), [0.0; 2]],
                        3,
                        b.edges,
                    )
                }
                Shape::Box { half } => (
                    [
                        [-half.x, -half.y],
                        [half.x, -half.y],
                        [half.x, half.y],
                        [-half.x, half.y],
                    ],
                    4,
                    15,
                ),
                Shape::Ball { .. } => continue,
            };
            if matches!(
                b.material,
                Material::Rock | Material::Masonry | Material::Panel
            ) {
                let a = Vec2::from(vertices[0]);
                let z = Vec2::from(vertices[1]);
                let c = Vec2::from(vertices[2]);
                *areas.entry(b.chunk).or_default() += (z - a).perp_dot(c - a).abs() * 0.5;
            }
            // Immutable shape revisions: later dents must not rewrite earlier
            // frames when the user rewinds or saves a comparison checkpoint.
            let geometry_id = if let Some(&(_, id)) = record
                .current_geometry
                .get(&b.id)
                .filter(|(revision, _)| *revision == b.shape_revision)
            {
                id
            } else {
                let id = record.geometry.len();
                record.current_geometry.insert(b.id, (b.shape_revision, id));
                record.geometry.push(Some(Geometry {
                    id,
                    color: b.color,
                    vertices,
                    count,
                    material: match b.material {
                        Material::Rock => 0,
                        Material::Masonry => 1,
                        Material::Panel => 2,
                        Material::Anchor => 3,
                        Material::Launcher => 4,
                        _ => 5,
                    },
                }));
                recorded_poses += 2; // Bound extra rest-shape storage too.
                id
            };
            poses.push(PoseRecord(
                geometry_id,
                b.position.to_array(),
                b.angle,
                edges,
                b.opacity,
                b.load,
            ));
        }
        let metrics = Metrics {
            ablated_area: stats.ablated_area,
            plastic_work: stats.plastic_work,
            plastic_strain: stats.plastic_strain,
            broken: stats.broken,
            chunks: stats.chunks,
            cells: stats.cells,
            cleared_area: 0.0 + stats.cleared_area,
            largest_area: areas.values().copied().fold(0.0, f32::max),
            fracture_work: stats.fracture_work,
            elastic_energy: peak_elastic,
            kinetic_energy: stats.kinetic_energy,
            unconverged: stats.unconverged,
        };
        recorded_poses += poses.len();
        record.frames.push(Frame {
            beam: w
                .laser_beam()
                .or_else(|| w.plasma_beam())
                .map(|b| (b.start.to_array(), b.end.to_array())),
            tick,
            poses,
            shells: w.shells().map(|s| s.position.to_array()).collect(),
            flashes: flashes.iter().map(|(p, r, _)| (*p, *r)).collect(),
            metrics,
        });
        recorded_poses
    }
}
