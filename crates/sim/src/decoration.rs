//! Lightweight scenery; geometry is only used for hit detection and drawing.
use crate::Vec2;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationKind {
    Shrub,
    Crate,
    Timber,
}

#[derive(Clone, Copy, Debug)]
pub struct DecorationView {
    pub id: usize,
    pub kind: DecorationKind,
    pub position: Vec2,
    pub angle: f32,
    pub size: Vec2,
}
impl DecorationView {
    pub fn outline(self) -> Vec<Vec2> {
        let rotation = Vec2::from_angle(self.angle);
        crate::level::rectangle(self.size * 0.5)
            .into_iter()
            .map(|p| self.position + rotation.rotate(p.into()))
            .collect()
    }
}
