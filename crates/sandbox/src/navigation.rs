use crate::view::Viewport;
use macroquad::prelude::*;

#[derive(Clone, Copy)]
pub enum LookAhead {
    Pointer(Vec2),
    Direction(Vec2),
}

pub struct Navigation {
    pub center: Vec2,
    pub zoom: f32,
    target_zoom: f32,
    anchor: Option<(Vec2, Vec2)>,
    look_offset: Vec2,
    look: LookAhead,
    follow_position: Option<Vec2>,
    game_camera_range: Option<f32>,
    expanded_game_view: bool,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            anchor: None,
            look_offset: Vec2::ZERO,
            look: LookAhead::Direction(Vec2::ZERO),
            follow_position: None,
            game_camera_range: None,
            expanded_game_view: false,
        }
    }
}

impl Navigation {
    /// Reveal the space formerly occupied by the HUD without enlarging the tank.
    /// A new match resets navigation, so this adjustment is applied once per match.
    pub fn expand_game_view(&mut self, rect: Rect) {
        if self.expanded_game_view {
            return;
        }
        let previous = Rect::new(rect.x, rect.y + 80.0, rect.w, rect.h - 177.0);
        let ratio = Viewport::new(previous, Vec2::ZERO, 1.0).scale
            / Viewport::new(rect, Vec2::ZERO, 1.0).scale;
        self.zoom *= ratio;
        self.target_zoom *= ratio;
        self.expanded_game_view = true;
    }

    pub fn fitted(center: Vec2, zoom: f32) -> Self {
        Self {
            center,
            zoom,
            target_zoom: zoom,
            anchor: None,
            look_offset: Vec2::ZERO,
            look: LookAhead::Direction(Vec2::ZERO),
            follow_position: None,
            game_camera_range: None,
            expanded_game_view: false,
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

    pub fn scroll_game(&mut self, wheel: f32, rect: Rect, sight_range: f32, radar: bool) {
        self.advance_game_zoom(0.0, rect, sight_range, radar);
        let (min, max) = Self::game_zoom_limits(rect, sight_range * if radar { 3.0 } else { 2.0 });
        self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(min, max);
        self.anchor = None;
    }

    pub fn advance_game_zoom(&mut self, dt: f32, rect: Rect, sight_range: f32, radar: bool) {
        // Twice the previous camera allowance, or three times inside friendly radar.
        let sight_range = sight_range * if radar { 3.0 } else { 2.0 };
        let (min, max) = Self::game_zoom_limits(rect, sight_range);
        if let Some(previous_range) = self.game_camera_range {
            // Preserve the player's relative zoom as terrain or friendly radar changes camera range.
            // Only retarget here: the post-physics call has dt=0 and must not snap.
            self.target_zoom *= previous_range / sight_range;
        } else {
            self.zoom = self.zoom.clamp(min, max);
        }
        self.game_camera_range = Some(sight_range);
        self.target_zoom = self.target_zoom.clamp(min, max);
        // The lower zoom limit is approached smoothly on leaving elevation or radar coverage.
        // The upper limit still protects tank visibility after a window resize.
        self.zoom = self.zoom.min(max);
        self.anchor = None;
        let blend = 1.0 - (-3.0 * dt).exp();
        self.zoom = (self.zoom.ln() + (self.target_zoom.ln() - self.zoom.ln()) * blend).exp();
        if (self.zoom - self.target_zoom).abs() < 0.00001 {
            self.zoom = self.target_zoom;
        }
    }

    pub fn follow_tank(&mut self, tank: Vec2, look: Option<LookAhead>, dt: f32, rect: Rect) {
        let scale = Viewport::new(rect, self.center, self.zoom).scale;
        let half_view = vec2(rect.w, rect.h) * 0.5 / scale;
        let safe_offset = (half_view - Vec2::splat(2.0)).max(Vec2::ZERO);
        if let Some(look) = look {
            self.look = look;
        }
        // Retain the last valid look over menus or outside the viewport.
        let desired = match self.look {
            LookAhead::Pointer(mouse) => {
                let direction = ((mouse - rect.center()) / (vec2(rect.w, rect.h) * 0.5))
                    .clamp(-Vec2::ONE, Vec2::ONE);
                // Ordinary mouse aiming near the middle of the screen does not pan.
                let direction = direction.signum()
                    * ((direction.abs() - Vec2::splat(0.45)).max(Vec2::ZERO) / 0.55);
                direction * (half_view.x * 0.22)
            }
            LookAhead::Direction(direction) => {
                let direction = direction.normalize_or_zero();
                // Put the vehicle near the rear of the view along the actual
                // barrel direction. Use the available space on both axes so
                // vertical and diagonal aim work on a wide handheld screen.
                let reach = (safe_offset / direction.abs().max(Vec2::splat(0.0001))).min_element();
                direction * reach * 0.85
            }
        }
        .clamp(-safe_offset, safe_offset);
        let look_speed = match self.look {
            LookAhead::Direction(_) => 3.0,
            LookAhead::Pointer(_) => 1.5,
        };
        self.look_offset = self
            .look_offset
            .lerp(desired, 1.0 - (-look_speed * dt).exp())
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
    fn full_game_view_reveals_more_world_without_changing_tank_scale() {
        for size in [
            vec2(1440.0, 900.0),
            vec2(1280.0, 800.0),
            vec2(1280.0, 720.0),
        ] {
            let full = Rect::new(0.0, 0.0, size.x, size.y);
            let old = Rect::new(0.0, 80.0, size.x, size.y - 177.0);
            let before = Viewport::new(old, Vec2::ZERO, 1.0);
            let mut camera = Navigation::default();
            camera.expand_game_view(full);
            camera.advance_game_zoom(0.0, full, 24.0, false);
            let after = Viewport::new(full, Vec2::ZERO, camera.zoom);
            assert!((after.scale - before.scale).abs() < 0.0001);
            assert!(after.rect.h / after.scale > before.rect.h / before.scale);
            let zoom = camera.zoom;
            camera.expand_game_view(full);
            assert_eq!(
                camera.zoom, zoom,
                "subsequent frames must not zoom out again"
            );
        }
    }

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
                camera.scroll_game(-1000.0, rect, sight, false);
                for _ in 0..120 {
                    camera.advance_game_zoom(1.0 / 60.0, rect, sight, false);
                }
                let scale = Viewport::new(rect, camera.center, camera.zoom).scale;
                assert!(rect.w.max(rect.h) / scale <= sight * 4.0 + 0.001);
            }
        }
    }

    #[test]
    fn radar_allows_three_times_previous_view_and_transitions_smoothly() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut camera = Navigation::default();
        camera.scroll_game(-1000.0, rect, 24.0, false);
        for _ in 0..240 {
            camera.advance_game_zoom(1.0 / 60.0, rect, 24.0, false);
        }
        let width =
            |camera: &Navigation| rect.w / Viewport::new(rect, camera.center, camera.zoom).scale;
        assert!((width(&camera) - 96.0).abs() < 0.01);
        for (radar, expected) in [(true, 144.0), (false, 96.0)] {
            let before = camera.zoom;
            camera.advance_game_zoom(0.0, rect, 24.0, radar);
            assert!((camera.zoom - before).abs() < 1e-6);
            camera.advance_game_zoom(1.0 / 60.0, rect, 24.0, radar);
            assert!((camera.zoom - before).abs() < before * 0.03);
            for _ in 0..240 {
                camera.advance_game_zoom(1.0 / 60.0, rect, 24.0, radar);
            }
            assert!((width(&camera) - expected).abs() < 0.05);
        }
    }

    #[test]
    fn gameplay_follows_translation_and_keeps_tank_inside_during_zoom_and_respawn() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut camera = Navigation::default();
        let mouse = vec2(rect.right(), rect.bottom());
        for _ in 0..120 {
            camera.follow_tank(
                Vec2::ZERO,
                Some(LookAhead::Pointer(mouse)),
                1.0 / 60.0,
                rect,
            );
        }
        let offset = camera.center;
        let tank = vec2(90.0, -75.0);
        camera.follow_tank(tank, Some(LookAhead::Pointer(mouse)), 0.0, rect);
        assert!(
            (camera.center - tank).distance(offset) < 0.0001,
            "respawn must snap to the tank"
        );
        for wheel in [1000.0, -1000.0] {
            camera.scroll_game(wheel, rect, 24.0, false);
            for _ in 0..120 {
                camera.advance_game_zoom(1.0 / 60.0, rect, 24.0, false);
                camera.follow_tank(tank, Some(LookAhead::Pointer(mouse)), 1.0 / 60.0, rect);
                let view = Viewport::new(rect, camera.center, camera.zoom);
                for corner in [vec2(-1.8, -1.8), vec2(1.8, 1.8)] {
                    assert!(rect.contains(view.screen(tank + corner)));
                }
            }
        }
        camera.advance_game_zoom(0.0, rect, 8.0, false);
        camera.follow_tank(Vec2::ZERO, None, 0.0, rect);
        assert!(rect.contains(Viewport::new(rect, camera.center, camera.zoom).screen(Vec2::ZERO)));
    }

    #[test]
    fn gameplay_look_ahead_is_bounded_and_frame_rate_independent() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut a = Navigation::default();
        let mut b = Navigation::default();
        let mouse = Some(LookAhead::Pointer(vec2(rect.right(), rect.center().y)));
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
    fn barrel_look_ahead_reveals_more_range_and_keeps_vehicle_visible() {
        for height in [800.0, 720.0] {
            let rect = Rect::new(0.0, 0.0, 1280.0, height);
            for zoom in [0.4, 1.0, 3.0] {
                for angle in [0.0, 0.7, -1.2, std::f32::consts::PI] {
                    let tank = vec2(-30.0, 12.0);
                    let direction = Vec2::from_angle(angle);
                    let mut camera = Navigation::fitted(tank, zoom);
                    camera.advance_game_zoom(0.0, rect, 24.0, false);
                    let scale = Viewport::new(rect, tank, camera.zoom).scale;
                    let half_view = vec2(rect.w, rect.h) * 0.5 / scale;
                    let centered_reach =
                        (half_view / direction.abs().max(Vec2::splat(0.0001))).min_element();
                    camera.follow_tank(tank, Some(LookAhead::Direction(direction)), 0.0, rect);
                    assert_eq!(camera.center, tank, "a direction change must not snap");
                    for _ in 0..240 {
                        camera.follow_tank(tank, None, 1.0 / 60.0, rect);
                    }
                    let offset = camera.center - tank;
                    assert!(offset.dot(direction) > 0.0);
                    assert!(offset.perp_dot(direction).abs() < 0.0001);
                    let view = Viewport::new(rect, camera.center, camera.zoom);
                    let gain = if zoom <= 1.0 { 1.4 } else { 1.05 };
                    assert!(rect.contains(view.screen(tank + direction * centered_reach * gain)));
                    for corner in [vec2(-1.8, -1.8), vec2(1.8, 1.8)] {
                        assert!(rect.contains(view.screen(tank + corner)));
                    }
                    let respawn = tank + vec2(100.0, -80.0);
                    camera.follow_tank(respawn, None, 0.0, rect);
                    assert!((camera.center - respawn).distance(offset) < 0.0001);
                }
            }
        }
    }

    #[test]
    fn barrel_camera_eases_turns_and_holds_aim_independently_of_frame_rate() {
        let rect = Rect::new(0.0, 0.0, 1280.0, 800.0);
        let mut positions = Vec::new();
        for fps in [30, 144] {
            let mut camera = Navigation::default();
            for direction in [Vec2::X, -Vec2::Y, -Vec2::X] {
                let before = camera.center;
                camera.follow_tank(Vec2::ZERO, Some(LookAhead::Direction(direction)), 0.0, rect);
                assert_eq!(camera.center, before);
                for _ in 0..fps {
                    camera.follow_tank(Vec2::ZERO, None, 1.0 / fps as f32, rect);
                }
            }
            positions.push(camera.center);
        }
        assert!(positions[0].distance(positions[1]) < 0.0001);
    }

    #[test]
    fn elevation_zoom_eases_in_both_directions_without_a_zero_dt_snap() {
        let rect = Rect::new(0.0, 80.0, 1440.0, 723.0);
        let mut camera = Navigation::default();
        camera.advance_game_zoom(0.0, rect, 24.0, false);
        let ground_zoom = camera.zoom;
        for (range, expected) in [(36.0, ground_zoom * 24.0 / 36.0), (24.0, ground_zoom)] {
            let before = camera.zoom;
            camera.advance_game_zoom(0.0, rect, range, false);
            assert!((camera.zoom - before).abs() < 1e-6);
            camera.advance_game_zoom(1.0 / 60.0, rect, range, false);
            assert!((camera.zoom - before).abs() > 0.0);
            assert!((camera.zoom - before).abs() < (expected - before).abs() * 0.1);
            let mut previous = camera.zoom;
            for _ in 0..240 {
                camera.advance_game_zoom(1.0 / 60.0, rect, range, false);
                assert!((camera.zoom - expected).abs() <= (previous - expected).abs() + 1e-6);
                previous = camera.zoom;
            }
            assert!((camera.zoom - expected).abs() < 0.0001);
        }
    }
}
