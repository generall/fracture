use crate::view::Viewport;
use macroquad::prelude::*;

pub struct Navigation {
    pub center: Vec2,
    pub zoom: f32,
    target_zoom: f32,
    anchor: Option<(Vec2, Vec2)>,
    look_offset: Vec2,
    look_direction: Vec2,
    follow_position: Option<Vec2>,
    game_sight_range: Option<f32>,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            anchor: None,
            look_offset: Vec2::ZERO,
            look_direction: Vec2::ZERO,
            follow_position: None,
            game_sight_range: None,
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
            look_offset: Vec2::ZERO,
            look_direction: Vec2::ZERO,
            follow_position: None,
            game_sight_range: None,
        }
    }

    pub fn scroll_level(&mut self, wheel: f32, mouse: Vec2, rect: Rect) {
        self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(0.03, 8.0);
        self.anchor = Some((
            Viewport::new(rect, self.center, self.zoom).world(mouse),
            mouse,
        ));
    }

    fn game_zoom_limits(rect: Rect, sight_range: f32) -> (f32, f32) {
        let base_scale = Viewport::new(rect, Vec2::ZERO, 1.0).scale;
        // Limit the larger screen dimension to twice the current sight range.
        let minimum = rect.w.max(rect.h) / (2.0 * sight_range * base_scale);
        // Leave room for the whole tank even at maximum zoom and with look-ahead.
        let maximum = (rect.w.min(rect.h) / (4.4 * base_scale)).min(8.0);
        (minimum.min(maximum), maximum)
    }

    pub fn scroll_game(&mut self, wheel: f32, rect: Rect, sight_range: f32) {
        self.advance_game_zoom(0.0, rect, sight_range);
        let (min, max) = Self::game_zoom_limits(rect, sight_range);
        self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(min, max);
        self.anchor = None;
    }

    pub fn advance_game_zoom(&mut self, dt: f32, rect: Rect, sight_range: f32) {
        let (min, max) = Self::game_zoom_limits(rect, sight_range);
        if let Some(previous_range) = self.game_sight_range {
            // Preserve the player's relative zoom as terrain changes sight.
            // Only retarget here: the post-physics call has dt=0 and must not snap.
            self.target_zoom *= previous_range / sight_range;
        } else {
            self.zoom = self.zoom.clamp(min, max);
        }
        self.game_sight_range = Some(sight_range);
        self.target_zoom = self.target_zoom.clamp(min, max);
        // The lower zoom limit is approached smoothly on leaving elevation.
        // The upper limit still protects tank visibility after a window resize.
        self.zoom = self.zoom.min(max);
        self.anchor = None;
        let blend = 1.0 - (-3.0 * dt).exp();
        self.zoom = (self.zoom.ln() + (self.target_zoom.ln() - self.zoom.ln()) * blend).exp();
        if (self.zoom - self.target_zoom).abs() < 0.00001 {
            self.zoom = self.target_zoom;
        }
    }

    pub fn follow_tank(&mut self, tank: Vec2, mouse: Option<Vec2>, dt: f32, rect: Rect) {
        let scale = Viewport::new(rect, self.center, self.zoom).scale;
        let half_view = vec2(rect.w, rect.h) * 0.5 / scale;
        let safe_offset = (half_view - Vec2::splat(2.0)).max(Vec2::ZERO);
        if let Some(mouse) = mouse {
            let direction = ((mouse - rect.center()) / (vec2(rect.w, rect.h) * 0.5))
                .clamp(-Vec2::ONE, Vec2::ONE);
            // Ordinary aiming near the middle of the screen does not pan.
            self.look_direction =
                direction.signum() * ((direction.abs() - Vec2::splat(0.45)).max(Vec2::ZERO) / 0.55);
        }
        // Keep the last valid direction over HUD panels or outside the viewport.
        // Equal world-space reach on both axes, independent of aspect ratio.
        let reach = Vec2::splat(half_view.x * 0.22);
        let desired = (self.look_direction * reach).clamp(-safe_offset, safe_offset);
        self.look_offset = self
            .look_offset
            .lerp(desired, 1.0 - (-1.5 * dt).exp())
            .clamp(-safe_offset, safe_offset);
        // Gentle follow easing softens acceleration and braking; a respawn snaps
        // immediately. The final clamp keeps the whole tank inside the view.
        let followed = match self.follow_position {
            Some(previous) if previous.distance(tank) < 12.0 => {
                previous.lerp(tank, 1.0 - (-3.0 * dt).exp())
            }
            _ => tank,
        };
        self.follow_position = Some(followed);
        self.center = (followed + self.look_offset).clamp(tank - safe_offset, tank + safe_offset);
        self.anchor = None;
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
    #[test]
    fn gameplay_zoom_out_is_bounded_by_sight_on_wide_and_tall_screens() {
        for rect in [
            Rect::new(0.0, 80.0, 1440.0, 723.0),
            Rect::new(0.0, 80.0, 2100.0, 723.0),
            Rect::new(0.0, 80.0, 800.0, 1000.0),
        ] {
            for sight in [8.0, 24.0, 36.0] {
                let mut camera = Navigation::default();
                camera.scroll_game(-1000.0, rect, sight);
                for _ in 0..120 {
                    camera.advance_game_zoom(1.0 / 60.0, rect, sight);
                }
                let scale = Viewport::new(rect, camera.center, camera.zoom).scale;
                assert!(rect.w.max(rect.h) / scale <= sight * 2.0 + 0.001);
            }
        }
    }

    #[test]
    fn gameplay_follows_translation_and_keeps_tank_inside_during_zoom_and_respawn() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut camera = Navigation::default();
        let mouse = vec2(rect.right(), rect.bottom());
        for _ in 0..120 {
            camera.follow_tank(Vec2::ZERO, Some(mouse), 1.0 / 60.0, rect);
        }
        let offset = camera.center;
        let tank = vec2(90.0, -75.0);
        camera.follow_tank(tank, Some(mouse), 0.0, rect);
        assert!(
            (camera.center - tank).distance(offset) < 0.0001,
            "respawn must snap to the tank"
        );
        for wheel in [1000.0, -1000.0] {
            camera.scroll_game(wheel, rect, 24.0);
            for _ in 0..120 {
                camera.advance_game_zoom(1.0 / 60.0, rect, 24.0);
                camera.follow_tank(tank, Some(mouse), 1.0 / 60.0, rect);
                let view = Viewport::new(rect, camera.center, camera.zoom);
                for corner in [vec2(-1.8, -1.8), vec2(1.8, 1.8)] {
                    assert!(rect.contains(view.screen(tank + corner)));
                }
            }
        }
        camera.advance_game_zoom(0.0, rect, 8.0);
        camera.follow_tank(Vec2::ZERO, None, 0.0, rect);
        assert!(rect.contains(Viewport::new(rect, camera.center, camera.zoom).screen(Vec2::ZERO)));
    }

    #[test]
    fn gameplay_look_ahead_is_bounded_and_frame_rate_independent() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut a = Navigation::default();
        let mut b = Navigation::default();
        let mouse = Some(vec2(rect.right(), rect.center().y));
        for _ in 0..30 {
            a.follow_tank(Vec2::ZERO, mouse, 1.0 / 30.0, rect);
        }
        for _ in 0..144 {
            b.follow_tank(Vec2::ZERO, mouse, 1.0 / 144.0, rect);
        }
        assert!(a.center.x > 0.0 && a.center.y.abs() < 0.0001);
        assert!(a.center.distance(b.center) < 0.0001);
    }
    #[test]
    fn elevation_zoom_eases_in_both_directions_without_a_zero_dt_snap() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut camera = Navigation::default();
        camera.advance_game_zoom(0.0, rect, 24.0);
        let ground_zoom = camera.zoom;
        for (range, expected) in [(36.0, ground_zoom * 24.0 / 36.0), (24.0, ground_zoom)] {
            let before = camera.zoom;
            camera.advance_game_zoom(0.0, rect, range);
            assert!((camera.zoom - before).abs() < 1e-6);
            camera.advance_game_zoom(1.0 / 60.0, rect, range);
            assert!((camera.zoom - before).abs() > 0.0);
            assert!((camera.zoom - before).abs() < (expected - before).abs() * 0.1);
            let mut previous = camera.zoom;
            for _ in 0..240 {
                camera.advance_game_zoom(1.0 / 60.0, rect, range);
                assert!((camera.zoom - expected).abs() <= (previous - expected).abs() + 1e-6);
                previous = camera.zoom;
            }
            assert!((camera.zoom - expected).abs() < 0.0001);
        }
    }
}
