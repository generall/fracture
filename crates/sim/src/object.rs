//! Portable object definitions. Authoring and simulation consume the same mesh.
use crate::{Material, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod library;
mod meshing;
pub use library::{Components, Crushable, Explosive, LibraryPreset, ObjectLibrary, Plasticity};
pub use meshing::{GrainLayout, GrainMode};

/// A smooth velocity-dependent restitution curve, independent of the fixture.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoftBounce {
    pub restitution: f32,
    /// Gentle coefficient up to this closing speed; base coefficient at twice it.
    pub speed: f32,
}
impl Default for SoftBounce {
    fn default() -> Self {
        Self {
            restitution: 0.64,
            speed: 16.0,
        }
    }
}
impl SoftBounce {
    pub(crate) fn coefficient(self, base: f32, closing: f32) -> f32 {
        if closing <= self.speed {
            return self.restitution;
        }
        if closing >= self.speed * 2.0 {
            return base;
        }
        let t = (closing - self.speed) / self.speed;
        let blend = t * t * (3.0 - 2.0 * t);
        self.restitution + (base - self.restitution) * blend
    }
}

/// An authoring guardrail, not a measured engine limit.
pub const MAX_FRAGMENTS: usize = 2048;
const MAX_JSON: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Rock,
    Masonry,
    Panel,
}
impl Surface {
    pub fn material(self) -> Material {
        match self {
            Self::Rock => Material::Rock,
            Self::Masonry => Material::Masonry,
            Self::Panel => Material::Panel,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Rock => "ROCK",
            Self::Masonry => "MASONRY",
            Self::Panel => "PANEL",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectAsset {
    pub version: u32,
    pub name: String,
    /// Closed, simple polygon in meters. Either winding is accepted; no holes.
    pub outline: Vec<[f32; 2]>,
    /// Authoring constraint only; the live plastic frame can still deform.
    #[serde(default, skip_serializing_if = "is_false")]
    pub circular: bool,
    pub surface: Surface,
    pub density: f32,
    pub thickness: f32,
    pub friction: f32,
    /// Collision restitution, from 0 (inelastic) to 1 (elastic).
    #[serde(default = "default_restitution")]
    pub restitution: f32,
    /// Optional stronger bounce for gentle impacts; hard impacts use restitution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_bounce: Option<SoftBounce>,
    /// Seam strengths in Pa, not bulk material measurements.
    pub tension: f32,
    pub compression: f32,
    pub shear: f32,
    /// Every refinement splits each triangle into four conforming triangles.
    pub refinement: u8,
    /// Strength and fracture-work multiplier inside each authored grain.
    pub cohesion: f32,
    /// Pin this outline edge to a fixed, breakable support.
    pub pinned_edge: Option<usize>,
    /// Further breakable edge supports. The original field remains compatible
    /// with existing object files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_pinned_edges: Vec<usize>,
    /// Replace continuous pinned edges with short breakable footing pads at this
    /// maximum spacing in metres. None preserves the original edge supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_spacing: Option<f32>,
    /// Clamp a central group of authored grains to the ground; detached chips are free.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ground_anchor: bool,
    /// Optional authored grain partition. Missing in original version-1 files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<GrainLayout>,
    /// Bulk elastic modulus in Pa; None uses the material preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub young: Option<f32>,
    /// Fracture-surface work in J/m²; None uses the material preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toughness: Option<f32>,
    /// Explicit RGB appearance, independent of physics material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Components::is_default")]
    pub components: Components,
}

#[derive(Clone, Copy)]
pub enum ObjectPreset {
    Rock,
    Wall,
    Plate,
    Bracket,
}
impl ObjectPreset {
    pub const ALL: [Self; 4] = [Self::Rock, Self::Wall, Self::Plate, Self::Bracket];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rock => "ROCK",
            Self::Wall => "WALL",
            Self::Plate => "PLATE",
            Self::Bracket => "L-BLOCK",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObjectMesh {
    pub vertices: Vec<Vec2>,
    pub triangles: Vec<[usize; 3]>,
    pub grains: Vec<usize>,
    pub area: f32,
}

impl ObjectMesh {
    pub fn ground_anchor_radius(&self) -> f32 {
        let lo = self
            .vertices
            .iter()
            .copied()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min);
        let hi = self
            .vertices
            .iter()
            .copied()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
        (hi - lo).min_element() * 0.25
    }

    pub(crate) fn ground_anchor_grains(&self) -> std::collections::BTreeSet<usize> {
        let (cell, center) = self.ground_anchor();
        let mut areas = BTreeMap::new();
        for (t, &grain) in self.triangles.iter().zip(&self.grains) {
            let [a, b, c] = t.map(|i| self.vertices[i]);
            let area = (b - a).perp_dot(c - a).abs() * 0.5;
            let entry = areas.entry(grain).or_insert((Vec2::ZERO, 0.0));
            entry.0 += (a + b + c) * (area / 3.0);
            entry.1 += area;
        }
        let radius = self.ground_anchor_radius();
        areas
            .into_iter()
            .filter(|(grain, (p, area))| {
                *grain == self.grains[cell] || (*p / *area).distance(center) <= radius
            })
            .map(|(grain, _)| grain)
            .collect()
    }

    /// Area centroid, or the nearest cell centroid for a concave outline whose
    /// centroid lies outside its material. The marker always lies inside a cell.
    pub fn ground_anchor(&self) -> (usize, Vec2) {
        let centers: Vec<_> = self
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.vertices[i]);
                ((a + b + c) / 3.0, (b - a).perp_dot(c - a).abs() * 0.5)
            })
            .collect();
        let center = centers.iter().map(|(p, area)| *p * *area).sum::<Vec2>()
            / centers.iter().map(|(_, area)| area).sum::<f32>();
        if let Some(i) = self.triangles.iter().position(|t| {
            let [a, b, c] = t.map(|i| self.vertices[i]);
            inside(center, a, b, c)
        }) {
            (i, center)
        } else {
            let i = centers
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.0.distance_squared(center)
                        .total_cmp(&b.0.distance_squared(center))
                })
                .unwrap()
                .0;
            (i, centers[i].0)
        }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

fn default_restitution() -> f32 {
    0.04 // Preserve the contact response of older object files.
}

impl ObjectAsset {
    pub fn preset(preset: ObjectPreset) -> Self {
        let (name, outline, surface, pinned_edge) = match preset {
            ObjectPreset::Rock => (
                "Basalt",
                vec![
                    [-1.5, -0.7],
                    [-0.7, -1.5],
                    [0.8, -1.3],
                    [1.6, -0.3],
                    [1.1, 1.1],
                    [-0.4, 1.5],
                    [-1.5, 0.5],
                ],
                Surface::Rock,
                None,
            ),
            ObjectPreset::Wall => (
                "Barrier",
                vec![[-0.7, -3.0], [0.7, -3.0], [0.7, 3.0], [-0.7, 3.0]],
                Surface::Masonry,
                Some(2),
            ),
            ObjectPreset::Plate => (
                "Plate",
                vec![[-1.7, -1.2], [1.7, -1.2], [1.7, 1.2], [-1.7, 1.2]],
                Surface::Panel,
                None,
            ),
            ObjectPreset::Bracket => (
                "Corner",
                vec![
                    [-1.7, -2.0],
                    [-0.5, -2.0],
                    [-0.5, 0.7],
                    [1.7, 0.7],
                    [1.7, 2.0],
                    [-1.7, 2.0],
                ],
                Surface::Masonry,
                Some(4),
            ),
        };
        let mut asset = Self {
            version: 1,
            name: name.into(),
            outline,
            circular: false,
            surface,
            density: 1.0,
            thickness: 0.25,
            friction: 0.65,
            restitution: default_restitution(),
            soft_bounce: None,
            tension: 1.0,
            compression: 1.0,
            shear: 1.0,
            refinement: 2,
            cohesion: 4.0,
            pinned_edge,
            additional_pinned_edges: Vec::new(),
            support_spacing: None,
            ground_anchor: false,
            layout: None,
            young: None,
            toughness: None,
            color: None,
            components: Components::default(),
        };
        asset.apply_surface(surface);
        asset
    }

    pub fn apply_surface(&mut self, surface: Surface) {
        self.surface = surface;
        self.density = surface.material().density();
        let s = surface.material().strength();
        self.tension = s.tension;
        self.compression = s.compression;
        self.shear = s.shear;
        self.young = None;
        self.toughness = None;
    }

    pub fn pinned_edges(&self) -> impl Iterator<Item = usize> + '_ {
        self.pinned_edge
            .into_iter()
            .chain(self.additional_pinned_edges.iter().copied())
    }

    pub fn support_segments(&self) -> Vec<[Vec2; 2]> {
        self.pinned_edges()
            .flat_map(|edge| {
                let a = Vec2::from(self.outline[edge]);
                let b = Vec2::from(self.outline[(edge + 1) % self.outline.len()]);
                let Some(spacing) = self.support_spacing else {
                    return vec![[a, b]];
                };
                let length = a.distance(b);
                let count = (length / spacing).ceil().max(1.0) as usize;
                let half = 0.4_f32.min(length / count as f32 * 0.25) / length;
                (0..count)
                    .map(|pad| {
                        let center = (pad as f32 + 0.5) / count as f32;
                        [a.lerp(b, center - half), a.lerp(b, center + half)]
                    })
                    .collect()
            })
            .collect()
    }

    pub fn toggle_pin(&mut self, edge: usize) {
        if self.pinned_edge == Some(edge) {
            self.pinned_edge = if self.additional_pinned_edges.is_empty() {
                None
            } else {
                Some(self.additional_pinned_edges.remove(0))
            };
        } else if let Some(i) = self.additional_pinned_edges.iter().position(|&e| e == edge) {
            self.additional_pinned_edges.remove(i);
        } else if self.pinned_edge.is_none() {
            self.pinned_edge = Some(edge);
        } else {
            self.additional_pinned_edges.push(edge);
        }
    }

    pub fn young_modulus(&self) -> f32 {
        self.young.unwrap_or(match self.surface {
            Surface::Rock => 40e9,
            Surface::Panel => 8e9,
            Surface::Masonry => 15e9,
        })
    }

    pub fn fracture_energy(&self) -> f32 {
        self.toughness.unwrap_or(match self.surface {
            Surface::Rock => 60.0,
            Surface::Panel => 120.0,
            Surface::Masonry => 30.0,
        })
    }

    /// New workshop defaults. `preset` retains the original reference specimens
    /// used by regression scenarios and legacy version-1 assets.
    pub fn workshop_preset(preset: ObjectPreset) -> Self {
        let mut asset = Self::preset(preset);
        let mode = if matches!(preset, ObjectPreset::Wall | ObjectPreset::Plate) {
            GrainMode::Bricks
        } else {
            GrainMode::Stones
        };
        asset.refinement = 0;
        asset.cohesion = 12.0;
        asset.regenerate_grains(mode, [0.7, 0.5], 7);
        asset
    }

    pub fn regenerate_grains(&mut self, mode: GrainMode, size: [f32; 2], seed: u32) {
        self.layout = Some(GrainLayout::generate(&self.outline, mode, size, seed));
    }

    pub fn is_circular(&self) -> bool {
        self.circular
            || self.name.eq_ignore_ascii_case("barrel")
            || self.name.eq_ignore_ascii_case("explosive barrel")
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.outline = (0..16)
            .map(|i| {
                (Vec2::from_angle(i as f32 * std::f32::consts::TAU / 16.0) * radius).to_array()
            })
            .collect();
        if let Some(layout) = &mut self.layout {
            layout.sites = vec![[0.0, 0.0]];
            layout.mode = GrainMode::Stones;
            layout.size = [radius * 2.0; 2];
        }
    }

    /// Keep large map instances within a predictable initial grain budget.
    /// This changes authored spacing, never the bulk material properties.
    pub fn fit_grain_budget(&mut self) {
        let Some(layout) = &self.layout else {
            return;
        };
        if self.is_circular() {
            return;
        }
        let (mode, mut size, seed) = (layout.mode, layout.size, layout.seed);
        if size.iter().any(|n| !n.is_finite() || *n < 0.15) {
            return;
        }
        self.refinement = self.refinement.min(2);
        let lo = self
            .outline
            .iter()
            .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(Vec2::from(*p)));
        let hi = self
            .outline
            .iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(Vec2::from(*p)));
        let extent = hi - lo;
        // Reserve space for staggered brick rows, concave boundaries and refinement.
        let budget = (96 / 4_usize.pow(self.refinement.min(2).into())).max(6);
        loop {
            let nx = (extent.x / size[0]).ceil().max(1.0) as usize;
            let ny = (extent.y / size[1]).ceil().max(1.0) as usize;
            if (nx + 1).saturating_mul(ny) <= budget {
                break;
            }
            size = size.map(|n| (n * 1.1).min(200.0));
        }
        self.regenerate_grains(mode, size, seed);
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_JSON {
            return Err("Object file exceeds 64 KiB.".into());
        }
        let object: Self =
            serde_json::from_str(json).map_err(|e| format!("Invalid object JSON: {e}"))?;
        object.mesh()?;
        Ok(object)
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.mesh()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn mesh(&self) -> Result<ObjectMesh, String> {
        self.mesh_with_bounds([4.5, 5.5])
    }

    /// Level instances may be larger than a workshop asset; all mesh budgets remain.
    pub fn level_mesh(&self) -> Result<ObjectMesh, String> {
        self.mesh_with_bounds([100.0, 100.0])
    }

    fn mesh_with_bounds(&self, bounds: [f32; 2]) -> Result<ObjectMesh, String> {
        self.components.validate()?;
        if let Some(bounce) = self.soft_bounce
            && (!bounce.restitution.is_finite()
                || !(0.0..=1.0).contains(&bounce.restitution)
                || !bounce.speed.is_finite()
                || !(0.1..=100.0).contains(&bounce.speed))
        {
            return Err("Soft bounce must be 0–1, and its speed 0.1–100 m/s.".into());
        }
        if self.version != 1 {
            return Err("Unsupported object version; expected 1.".into());
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 32
            || self.name.chars().any(char::is_control)
        {
            return Err("Name must contain 1–32 printable characters.".into());
        }
        for (label, value, min, max) in [
            ("Density", self.density, 100.0, 20_000.0),
            ("Thickness", self.thickness, 0.02, 2.0),
            ("Friction", self.friction, 0.0, 2.0),
            ("Bounciness", self.restitution, 0.0, 1.0),
            ("Tension", self.tension, 1000.0, 1e9),
            ("Compression", self.compression, 1000.0, 1e9),
            ("Shear", self.shear, 1000.0, 1e9),
            ("Grain cohesion", self.cohesion, 1.0, 100.0),
            ("Young's modulus", self.young_modulus(), 1e5, 1e12),
            ("Fracture energy", self.fracture_energy(), 0.1, 100_000.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{label} must be between {min} and {max}."));
            }
        }
        let n = self.outline.len();
        if !(3..=128).contains(&n) {
            return Err("Use 3–128 outline vertices.".into());
        }
        if self.is_circular() {
            let radius = Vec2::from(self.outline[0]).length();
            if n != 16
                || self.outline.iter().enumerate().any(|(i, p)| {
                    Vec2::from(*p).distance(
                        Vec2::from_angle(i as f32 * std::f32::consts::TAU / 16.0) * radius,
                    ) > radius * 0.001
                })
            {
                return Err("Barrels must have a circular outline; resize their diameter.".into());
            }
        }
        if self.refinement > 4
            || (self.layout.is_none()
                && (n - 2) * 4_usize.pow(self.refinement.into()) > MAX_FRAGMENTS)
        {
            return Err(format!(
                "Mesh exceeds {MAX_FRAGMENTS} cells; reduce detail or vertices."
            ));
        }
        if let Some(spacing) = self.support_spacing
            && (!spacing.is_finite() || !(1.0..=20.0).contains(&spacing))
        {
            return Err("Support spacing must be between 1 and 20 metres.".into());
        }
        if self.pinned_edges().any(|e| e >= n) {
            return Err("Pinned edge does not exist.".into());
        }
        let pins: Vec<_> = self.pinned_edges().collect();
        if pins.iter().enumerate().any(|(i, e)| pins[..i].contains(e)) {
            return Err("Each edge may have only one support.".into());
        }
        let mut vertices: Vec<_> = self.outline.iter().map(|&p| Vec2::from(p)).collect();
        if vertices
            .iter()
            .any(|p| !p.is_finite() || p.x.abs() > bounds[0] || p.y.abs() > bounds[1])
        {
            return Err(format!(
                "Keep the outline inside ±{} m X and ±{} m Y.",
                bounds[0], bounds[1]
            ));
        }
        for i in 0..n {
            let (a, b) = (vertices[i], vertices[(i + 1) % n]);
            if a.distance(b) < 0.05 {
                return Err("Outline edges must be at least 0.05 m long.".into());
            }
            let c = vertices[(i + 2) % n];
            if (b - a).perp_dot(c - b).abs() < 1e-5 && (b - a).dot(c - b) < 0.0 {
                return Err("An outline edge doubles back on itself.".into());
            }
            for j in i + 1..n {
                if j == i + 1 || (i == 0 && j == n - 1) {
                    continue;
                }
                if intersects(a, b, vertices[j], vertices[(j + 1) % n]) {
                    return Err("Outline crosses itself. Move or remove a vertex.".into());
                }
            }
        }
        let signed_area = (0..n)
            .map(|i| vertices[i].perp_dot(vertices[(i + 1) % n]))
            .sum::<f32>()
            * 0.5;
        if signed_area.abs() < 0.05 {
            return Err("Object area must be at least 0.05 m².".into());
        }
        let mut polygon: Vec<_> = (0..n).collect();
        if signed_area < 0.0 {
            polygon.reverse();
        }
        let mut triangles = Vec::new();
        // Ear clipping supports concave polygons. Boundary points block ears,
        // preserving collinear authored vertices and shared seam topology.
        while polygon.len() > 3 {
            let ear = (0..polygon.len())
                .find(|&i| {
                    let ids = [
                        polygon[(i + polygon.len() - 1) % polygon.len()],
                        polygon[i],
                        polygon[(i + 1) % polygon.len()],
                    ];
                    let [a, b, c] = ids.map(|j| vertices[j]);
                    (b - a).perp_dot(c - a) > 1e-5
                        && !polygon
                            .iter()
                            .any(|j| !ids.contains(j) && inside(vertices[*j], a, b, c))
                })
                .ok_or("Outline has a sliver too narrow to triangulate.")?;
            triangles.push([
                polygon[(ear + polygon.len() - 1) % polygon.len()],
                polygon[ear],
                polygon[(ear + 1) % polygon.len()],
            ]);
            polygon.remove(ear);
        }
        triangles.push([polygon[0], polygon[1], polygon[2]]);
        let mut grains: Vec<_> = (0..triangles.len()).collect();
        if let Some(layout) = &self.layout {
            (vertices, triangles, grains) =
                meshing::partition(&vertices, &triangles, layout, bounds)?;
        }
        if triangles.len() * 4_usize.pow(self.refinement.into()) > MAX_FRAGMENTS {
            return Err(format!(
                "Mesh exceeds {MAX_FRAGMENTS} cells; use larger grains or less detail."
            ));
        }
        for _ in 0..self.refinement {
            let mut midpoints = BTreeMap::new();
            let mut refined = Vec::new();
            let mut next_grains = Vec::new();
            for (&[a, b, c], &grain) in triangles.iter().zip(&grains) {
                let [ab, bc, ca] = [(a, b), (b, c), (c, a)].map(|(a, b)| {
                    *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                        let id = vertices.len();
                        vertices.push((vertices[a] + vertices[b]) * 0.5);
                        id
                    })
                });
                refined.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
                next_grains.extend([grain; 4]);
            }
            triangles = refined;
            grains = next_grains;
        }
        // Very slender collision polygons are unstable even when the outline is
        // valid. Test the smallest altitude rather than only triangle area.
        if let Some(t) = triangles.iter().find(|t| {
            let [a, b, c] = t.map(|i| vertices[i]);
            (b - a).perp_dot(c - a).abs() / a.distance(b).max(b.distance(c)).max(c.distance(a))
                < if self.layout.is_some() { 0.0005 } else { 0.008 }
        }) {
            return Err(format!(
                "Mesh contains a very thin cell near ({:.2}, {:.2}); move a grain point, use larger grains or reduce detail.",
                vertices[t[0]].x, vertices[t[0]].y
            ));
        }
        Ok(ObjectMesh {
            vertices,
            triangles,
            grains,
            area: signed_area.abs(),
        })
    }
}

fn inside(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    (b - a).perp_dot(p - a) >= -1e-6
        && (c - b).perp_dot(p - b) >= -1e-6
        && (a - c).perp_dot(p - c) >= -1e-6
}
fn intersects(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let on = |p: Vec2, x: Vec2, y: Vec2| {
        (y - x).perp_dot(p - x).abs() < 1e-6 && (p - x).dot(p - y) <= 1e-6
    };
    let ab = (b - a).perp_dot(c - a) * (b - a).perp_dot(d - a);
    let cd = (d - c).perp_dot(a - c) * (d - c).perp_dot(b - c);
    (ab < 0.0 && cd < 0.0) || on(a, c, d) || on(b, c, d) || on(c, a, b) || on(d, a, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ground_and_multiple_edge_supports_are_optional_and_validated() {
        let legacy = super::ObjectAsset::preset(super::ObjectPreset::Wall);
        let json = legacy.to_json().unwrap();
        assert!(!json.contains("ground_anchor") && !json.contains("additional_pinned_edges"));
        assert_eq!(super::ObjectAsset::from_json(&json).unwrap(), legacy);
        let mut a = super::LibraryPreset::Wall.asset();
        a.additional_pinned_edges.push(2);
        assert!(a.mesh().is_err(), "duplicate support should be rejected");
        a.additional_pinned_edges = vec![a.outline.len()];
        assert!(a.mesh().is_err(), "out-of-range support should be rejected");
        a = super::LibraryPreset::Wall.asset();
        a.toggle_pin(0);
        assert_eq!(a.pinned_edges().collect::<Vec<_>>(), vec![2, 0]);
        a.toggle_pin(0);
        assert_eq!(a.pinned_edges().collect::<Vec<_>>(), vec![2]);
        a.toggle_pin(0);
        a.toggle_pin(2);
        assert_eq!(a.pinned_edges().collect::<Vec<_>>(), vec![0]);
        assert!(a.mesh().is_ok());
        let mesh = super::ObjectAsset::workshop_preset(super::ObjectPreset::Bracket)
            .mesh()
            .unwrap();
        let (cell, point) = mesh.ground_anchor();
        let [x, y, z] = mesh.triangles[cell].map(|i| mesh.vertices[i]);
        assert!(
            super::inside(point, x, y, z),
            "concave object attachment must lie in its material"
        );
    }

    use super::*;
    #[test]
    fn concave_mesh_preserves_area_and_has_conforming_edges() {
        for preset in ObjectPreset::ALL {
            let asset = ObjectAsset::preset(preset);
            let mesh = asset.mesh().unwrap();
            let area: f32 = mesh
                .triangles
                .iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| mesh.vertices[i]);
                    (b - a).perp_dot(c - a).abs() * 0.5
                })
                .sum();
            assert!((area - mesh.area).abs() < 1e-4);
            let mut edges = BTreeMap::new();
            for t in &mesh.triangles {
                for i in 0..3 {
                    let (a, b) = (t[i], t[(i + 1) % 3]);
                    *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                }
            }
            assert!(edges.values().all(|&n| n == 1 || n == 2));
            assert_eq!(
                edges.values().filter(|&&n| n == 1).count(),
                asset.outline.len() * 2_usize.pow(asset.refinement.into())
            );
            assert_eq!(
                ObjectAsset::from_json(&asset.to_json().unwrap()).unwrap(),
                asset
            );
        }
    }
    #[test]
    fn rejects_bad_geometry_and_unbounded_imports() {
        let mut a = ObjectAsset::preset(ObjectPreset::Plate);
        a.outline.swap(1, 2);
        assert!(a.mesh().unwrap_err().contains("crosses"));
        a = ObjectAsset::preset(ObjectPreset::Plate);
        a.density = f32::NAN;
        assert!(a.mesh().is_err());
        a = ObjectAsset::preset(ObjectPreset::Plate);
        a.refinement = 255;
        assert!(a.mesh().is_err());
        a = ObjectAsset::preset(ObjectPreset::Plate);
        a.pinned_edge = Some(4);
        assert!(a.mesh().is_err());
        assert!(ObjectAsset::from_json(&" ".repeat(65_537)).is_err());
    }
    #[test]
    fn reversed_winding_and_inserted_edge_vertex_are_valid() {
        let mut a = ObjectAsset::preset(ObjectPreset::Plate);
        a.outline.insert(1, [0.0, -1.2]);
        let area = a.mesh().unwrap().area;
        a.outline.reverse();
        assert!((a.mesh().unwrap().area - area).abs() < 1e-5);
    }
}
