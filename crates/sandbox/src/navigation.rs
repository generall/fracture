use crate::view::Viewport;
use macroquad::prelude::*;

pub struct Navigation {
    pub center: Vec2,
    pub zoom: f32,
    target_zoom: f32,
    anchor: Option<(Vec2, Vec2)>,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            anchor: None,
        }
    }
}

impl Navigation {
    pub fn fitted(center: Vec2, zoom: f32) -> Self {
        Self {
            center,
            zoom,
            target_zoom: zoom,
            anchor: None,
        }
    }

    pub fn scroll_level(&mut self, wheel: f32, mouse: Vec2, rect: Rect) {
        self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(0.03, 8.0);
        self.anchor = Some((
            Viewport::new(rect, self.center, self.zoom).world(mouse),
            mouse,
        ));
    }

    pub fn scroll(&mut self, wheel: f32, mouse: Vec2, rect: Rect) {
        self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(0.65, 3.5);
        self.anchor = Some((
            Viewport::new(rect, self.center, self.zoom).world(mouse),
            mouse,
        ));
    }

    pub fn pan(&mut self, delta: Vec2, rect: Rect) {
        self.center -= delta / Viewport::new(rect, self.center, self.zoom).scale;
        self.anchor = None;
    }

    pub fn advance(&mut self, dt: f32, rect: Rect) {
        // Exponential easing in log space is frame-rate independent and gives
        // the same response when zooming in and out. Camera time is wall time.
        let blend = 1.0 - (-14.0 * dt).exp();
        self.zoom = (self.zoom.ln() + (self.target_zoom.ln() - self.zoom.ln()) * blend).exp();
        if (self.zoom - self.target_zoom).abs() < 0.00001 {
            self.zoom = self.target_zoom;
        }
        if let Some((world, screen)) = self.anchor {
            let scale = Viewport::new(rect, self.center, self.zoom).scale;
            self.center = world - (screen - rect.center()) / scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_eases_without_moving_the_cursor_anchor() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mouse = vec2(925.0, 380.0);
        let mut camera = Navigation::default();
        let before = Viewport::new(rect, camera.center, camera.zoom).world(mouse);
        camera.scroll(3.0, mouse, rect);
        let mut previous = 1.0;
        for _ in 0..60 {
            camera.advance(1.0 / 120.0, rect);
            assert!(camera.zoom >= previous && camera.zoom - previous < 0.05);
            let anchored = Viewport::new(rect, camera.center, camera.zoom).world(mouse);
            assert!(anchored.distance(before) < 0.00001);
            previous = camera.zoom;
        }
        assert!((camera.zoom - 0.33_f32.exp()).abs() < 0.001);
    }

    #[test]
    fn zoom_response_is_independent_of_frame_rate() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut a = Navigation::default();
        let mut b = Navigation::default();
        a.scroll(-2.0, rect.center(), rect);
        b.scroll(-2.0, rect.center(), rect);
        for _ in 0..15 {
            a.advance(1.0 / 30.0, rect);
        }
        for _ in 0..72 {
            b.advance(1.0 / 144.0, rect);
        }
        assert!((a.zoom - b.zoom).abs() < 0.00001);
    }
}
