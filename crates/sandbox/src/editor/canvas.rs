//! Mesh rendering, camera interaction and direct geometry edits.
use super::*;

impl Editor {
    pub(super) fn insert_vertex(&mut self, edge: usize, point: Vec2) {
        if self.asset.is_circular() {
            self.status = "Barrels stay circular. Drag a rim point to change the diameter.".into();
            return;
        }
        if self.asset.outline.len() >= 128 {
            self.status = "Maximum 128 outline vertices.".into();
            return;
        }
        self.checkpoint();
        self.asset.outline.insert(edge + 1, point.to_array());
        self.selected = edge + 1;
        // An edited support edge needs an explicit new choice.
        self.asset.pinned_edge = self.asset.pinned_edge.and_then(|pin| {
            if pin == edge {
                None
            } else {
                Some(pin + usize::from(pin > edge))
            }
        });
        self.asset.additional_pinned_edges.retain_mut(|pin| {
            if *pin == edge {
                false
            } else {
                *pin += usize::from(*pin > edge);
                true
            }
        });
    }

    pub(super) fn delete_vertex(&mut self) {
        if self.grain_edit {
            if self
                .asset
                .layout
                .as_ref()
                .is_some_and(|l| l.sites.len() > 1)
            {
                self.checkpoint();
                let sites = &mut self.asset.layout.as_mut().unwrap().sites;
                sites.remove(self.selected.min(sites.len() - 1));
                self.selected = self.selected.min(sites.len() - 1);
            }
            return;
        }
        if self.asset.is_circular() {
            self.status = "Barrels keep their circular rim.".into();
            return;
        }
        if self.asset.outline.len() <= 3 {
            self.status = "A polygon needs at least three vertices.".into();
            return;
        }
        self.checkpoint();
        self.asset.outline.remove(self.selected);
        self.asset.pinned_edge = None;
        self.asset.additional_pinned_edges.clear();
        self.selected = self.selected.min(self.asset.outline.len() - 1);
    }

    pub(super) fn fit(&mut self) {
        self.fit_requested = true;
        self.zoom_anchor = None;
    }

    pub(super) fn canvas(
        &mut self,
        rect: Rect,
        mouse: Vec2,
        dt: f32,
        pixel_scale: f32,
        editing: bool,
    ) {
        let base = (rect.w / 10.0).min(rect.h / 7.0);
        if self.fit_requested {
            let mut min = vec2(f32::INFINITY, f32::INFINITY);
            let mut max = vec2(f32::NEG_INFINITY, f32::NEG_INFINITY);
            for &point in &self.asset.outline {
                min = min.min(Vec2::from(point));
                max = max.max(Vec2::from(point));
            }
            self.center = (min + max) * 0.5;
            let extent = (max - min).max(vec2(0.1, 0.1));
            self.zoom = ((rect.w - 180.0) / extent.x).min((rect.h - 140.0) / extent.y) / base;
            self.zoom = self.zoom.clamp(0.5, 2.0);
            self.target_zoom = self.zoom;
            self.fit_requested = false;
        }
        let screen = |p: Vec2, center: Vec2, zoom: f32| rect.center() + (p - center) * base * zoom;
        let world = |p: Vec2, center: Vec2, zoom: f32| center + (p - rect.center()) / (base * zoom);
        if rect.contains(mouse) {
            if is_mouse_button_down(MouseButton::Right) {
                self.center -= (mouse - self.last_mouse) / (base * self.zoom);
                self.zoom_anchor = None;
            }
            let wheel = mouse_wheel().1;
            if wheel != 0.0 {
                self.zoom_anchor = Some((world(mouse, self.center, self.zoom), mouse));
                self.target_zoom = (self.target_zoom * (wheel * 0.11).exp()).clamp(0.5, 4.0);
            }
        }
        self.zoom = (self.zoom.ln()
            + (self.target_zoom.ln() - self.zoom.ln()) * (1.0 - (-14.0 * dt).exp()))
        .exp();
        if let Some((anchor, cursor)) = self.zoom_anchor {
            self.center = anchor - (cursor - rect.center()) / (base * self.zoom);
        }
        if !editing && rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
            let editable = if self.grain_edit {
                &self.asset.layout.as_ref().unwrap().sites
            } else {
                &self.asset.outline
            };
            let nearest = editable
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    (
                        i,
                        screen(Vec2::from(p), self.center, self.zoom).distance(mouse),
                    )
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, distance)) = nearest
                && distance < 14.0
            {
                self.selected = i;
                self.checkpoint();
                self.dragging = true;
            } else if self.grain_edit
                && (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
            {
                self.add_grain(
                    world(mouse, self.center, self.zoom).clamp(vec2(-4.5, -5.5), vec2(4.5, 5.5)),
                );
            } else if !self.grain_edit
                && (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
            {
                let nearest = (0..self.asset.outline.len())
                    .map(|i| {
                        let a = Vec2::from(self.asset.outline[i]);
                        let b = Vec2::from(self.asset.outline[(i + 1) % self.asset.outline.len()]);
                        let p = world(mouse, self.center, self.zoom);
                        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.05, 0.95);
                        let point = a + (b - a) * t;
                        (
                            i,
                            point,
                            screen(point, self.center, self.zoom).distance(mouse),
                        )
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((i, p, d)) = nearest
                    && d < 20.0
                {
                    self.insert_vertex(i, p);
                }
            }
        }
        if self.dragging {
            if is_mouse_button_down(MouseButton::Left) {
                let mut p =
                    world(mouse, self.center, self.zoom).clamp(vec2(-4.5, -5.5), vec2(4.5, 5.5));
                if self.snap {
                    p = (p * 10.0).round() / 10.0;
                }
                self.set_point(p.to_array());
            } else {
                self.dragging = false;
            }
        }
        self.refresh();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (rect.x * pixel_scale) as i32,
                (rect.y * pixel_scale) as i32,
                (rect.w * pixel_scale) as i32,
                (rect.h * pixel_scale) as i32,
            )));
        }
        let min = world(vec2(rect.x, rect.y), self.center, self.zoom);
        let max = world(vec2(rect.right(), rect.bottom()), self.center, self.zoom);
        for x in (min.x.floor() as i32)..=(max.x.ceil() as i32) {
            let p = screen(vec2(x as f32, 0.0), self.center, self.zoom);
            draw_line(
                p.x,
                rect.y,
                p.x,
                rect.bottom(),
                1.0,
                if x == 0 {
                    RULE
                } else {
                    Color::new(0.08, 0.12, 0.20, 1.0)
                },
            );
        }
        for y in (min.y.floor() as i32)..=(max.y.ceil() as i32) {
            let p = screen(vec2(0.0, y as f32), self.center, self.zoom);
            draw_line(
                rect.x,
                p.y,
                rect.right(),
                p.y,
                1.0,
                if y == 0 {
                    RULE
                } else {
                    Color::new(0.08, 0.12, 0.20, 1.0)
                },
            );
        }
        let outline_color = if self.mesh.is_ok() {
            crate::object_library::asset_color(&self.asset)
        } else {
            PINK
        };
        if let Ok(mesh) = &self.mesh {
            for (i, t) in mesh.triangles.iter().enumerate() {
                let [a, b, c] = t.map(|i| {
                    screen(
                        vec2(mesh.vertices[i].x, mesh.vertices[i].y),
                        self.center,
                        self.zoom,
                    )
                });
                draw_triangle(
                    a,
                    b,
                    c,
                    Color::new(0.09, 0.12, 0.22, 0.20 + (mesh.grains[i] % 3) as f32 * 0.07),
                );
                if self.show_mesh {
                    draw_triangle_lines(a, b, c, 1.0, Color::new(0.20, 0.25, 0.38, 0.6));
                }
            }
        }
        if let Ok(mesh) = &self.mesh {
            let mut edges = std::collections::BTreeMap::new();
            for (i, t) in mesh.triangles.iter().enumerate() {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    edges
                        .entry((a.min(b), a.max(b)))
                        .or_insert_with(Vec::new)
                        .push(mesh.grains[i]);
                }
            }
            for ((a, b), grains) in edges {
                if grains.len() == 1 || grains[0] != grains[1] {
                    let a = screen(
                        Vec2::from(mesh.vertices[a].to_array()),
                        self.center,
                        self.zoom,
                    );
                    let b = screen(
                        Vec2::from(mesh.vertices[b].to_array()),
                        self.center,
                        self.zoom,
                    );
                    draw_line(a.x, a.y, b.x, b.y, 1.8, outline_color);
                }
            }
        }
        if self.grain_edit
            && let Some(layout) = &self.asset.layout
        {
            for (i, p) in layout.sites.iter().enumerate() {
                let point = screen(Vec2::from(*p), self.center, self.zoom);
                draw_circle(point.x, point.y, 5.5, PAPER);
                draw_circle_lines(
                    point.x,
                    point.y,
                    5.5,
                    2.0,
                    if i == self.selected { INK } else { CYAN },
                );
            }
        }
        if self.asset.ground_anchor
            && let Ok(mesh) = &self.mesh
        {
            let (_, point) = mesh.ground_anchor();
            let p = screen(Vec2::from(point.to_array()), self.center, self.zoom);
            draw_circle_lines(
                p.x,
                p.y,
                mesh.ground_anchor_radius() * base * self.zoom,
                1.0,
                VIOLET,
            );
            draw_circle_lines(p.x, p.y, 8.0, 2.0, VIOLET);
            draw_line(p.x - 12.0, p.y, p.x + 12.0, p.y, 2.0, VIOLET);
            draw_line(p.x, p.y - 12.0, p.x, p.y + 12.0, 2.0, VIOLET);
            text("GROUND", p.x + 16.0, p.y - 12.0, 16.0, VIOLET);
        }
        for (i, &p) in self.asset.outline.iter().enumerate() {
            let a = screen(Vec2::from(p), self.center, self.zoom);
            let b = screen(
                Vec2::from(self.asset.outline[(i + 1) % self.asset.outline.len()]),
                self.center,
                self.zoom,
            );
            let pin = self.asset.pinned_edges().any(|e| e == i);
            draw_line(
                a.x,
                a.y,
                b.x,
                b.y,
                if pin { 5.0 } else { 2.0 },
                if pin {
                    VIOLET
                } else if self.tab == Tab::Shape && i == self.selected {
                    INK
                } else {
                    outline_color
                },
            );
            if self.tab == Tab::Shape {
                draw_circle(
                    a.x,
                    a.y,
                    if !self.grain_edit && i == self.selected {
                        7.0
                    } else {
                        5.0
                    },
                    PAPER,
                );
                draw_circle_lines(
                    a.x,
                    a.y,
                    if !self.grain_edit && i == self.selected {
                        7.0
                    } else {
                        5.0
                    },
                    2.0,
                    if !self.grain_edit && i == self.selected {
                        INK
                    } else {
                        outline_color
                    },
                );
                text(
                    &format!("{:02}", i + 1),
                    a.x + 12.0,
                    a.y - 10.0,
                    16.0,
                    if !self.grain_edit && i == self.selected {
                        INK
                    } else {
                        MUTED
                    },
                );
            }
            if pin {
                text(
                    "PIN",
                    (a.x + b.x) * 0.5 + 10.0,
                    (a.y + b.y) * 0.5 - 12.0,
                    16.0,
                    VIOLET,
                );
            }
        }
        let bar = base * self.zoom;
        draw_line(
            rect.x + 20.0,
            rect.bottom() - 25.0,
            rect.x + 20.0 + bar,
            rect.bottom() - 25.0,
            2.0,
            INK,
        );
        text("1 m", rect.x + 20.0, rect.bottom() - 36.0, 16.0, MUTED);
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, RULE);
    }
}
