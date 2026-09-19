//! Fog only selects live figures and retains last-observed render data.
//! Both are drawn by the ordinary world renderer at the current camera scale.
mod figures;
use crate::view::scene::Scene;

#[derive(Default)]
pub struct Fog {
    figures: figures::Figures,
    scene: Scene,
}
impl Fog {
    pub fn observe(&mut self, world: &sim::World, vision: &sim::Vision, now: f64) {
        self.scene = self.figures.update(world, vision, now).scene;
        // The player's own tank is always known.
        self.scene
            .bodies
            .extend(world.bodies().filter(|b| b.id == world.player_tank_id()));
        self.scene
            .tanks
            .extend(world.tanks().filter(|t| t.id == world.player_tank_id()));
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
}
