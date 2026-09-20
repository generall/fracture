use super::*;

impl LevelEditor {
    fn refresh_geometry(&mut self) {
        if self
            .geometry
            .as_ref()
            .is_some_and(|(level, _)| level == &self.level)
        {
            return;
        }
        let meshes = self.level.resolved_meshes().unwrap_or_else(|error| {
            self.status = error;
            vec![None; self.level.items.len()]
        });
        self.geometry = Some((self.level.clone(), meshes));
    }
    fn item_mesh(&self, index: usize) -> Option<&sim::ObjectMesh> {
        self.geometry.as_ref()?.1.get(index)?.as_ref()
    }

    fn point(&self, p: Vec2) -> Vec2 {
        if self.snap && !self.tool.draws_cells() {
            (p * 4.0).round() / 4.0
        } else {
            p
        }
    }
    pub(super) fn draw_canvas(
        &mut self,
        rect: Rect,
        mouse: Vec2,
        dt: f32,
        pixels: f32,
        input: bool,
    ) {
        self.refresh_geometry();
        if self.fit_requested {
            let base = Viewport::new(rect, Vec2::ZERO, 1.0).scale;
            let fit =
                ((rect.w - 40.0) / self.level.size[0]).min((rect.h - 40.0) / self.level.size[1]);
            self.navigation = Navigation::fitted(Vec2::ZERO, fit / base);
            self.fit_requested = false;
        }
        if input && rect.contains(mouse) {
            if is_mouse_button_down(MouseButton::Right) {
                self.navigation.pan(mouse - self.last_mouse, rect);
            }
            if mouse_wheel().1 != 0.0 {
                self.navigation.scroll_level(mouse_wheel().1, mouse, rect);
            }
        }
        self.navigation.advance(dt, rect);
        let view = Viewport::new(rect, self.navigation.center, self.navigation.zoom);
        let point = self.point(view.world(mouse));
        if input {
            if is_key_pressed(KeyCode::Escape)
                && let Some(drag) = self.drag.take()
            {
                self.level = drag.before;
                self.selection = drag.selection;
                self.clamp_selection();
            }
            if is_mouse_button_pressed(MouseButton::Left) && rect.contains(mouse) {
                self.begin_drag(&view, mouse, point);
            }
            if self.drag.is_some() {
                self.update_drag(point);
            }
            if is_mouse_button_released(MouseButton::Left)
                && let Some(drag) = self.drag.take()
            {
                if matches!(drag.kind, DragKind::Erase) {
                    let (min, max, _) =
                        grid_rectangle(drag.start, point, self.level.size.into(), false);
                    if let Err(error) = self.level.erase_rectangle(s(min), s(max)) {
                        self.level = drag.before.clone();
                        self.status = error;
                    }
                    self.clear_selection();
                }
                if !matches!(drag.kind, DragKind::Marquee | DragKind::Erase)
                    && self.level != drag.before
                    && !is_key_down(KeyCode::LeftAlt)
                    && !is_key_down(KeyCode::RightAlt)
                {
                    self.join_touching();
                }
                self.finish_change(drag.before);
            }
        }
        self.refresh_geometry();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (rect.x * pixels) as i32,
                (rect.y * pixels) as i32,
                (rect.w * pixels) as i32,
                (rect.h * pixels) as i32,
            )));
        }
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
        let half = Vec2::from(self.level.size) * 0.5;
        let a = view.screen(-half);
        let b = view.screen(half);
        draw_rectangle_lines(a.x, a.y, b.x - a.x, b.y - a.y, 2.0, MUTED);
        let terrain: Vec<_> = self
            .level
            .items
            .iter()
            .filter_map(|item| {
                if let LevelItem::Terrain { terrain, min, max } = item {
                    Some(sim::TerrainPatch {
                        kind: *terrain,
                        min: (*min).into(),
                        max: (*max).into(),
                    })
                } else {
                    None
                }
            })
            .collect();
        if self.tool.draws_cells() {
            crate::terrain_view::draw(&terrain, &view);
        }
        let step = if view.scale < 8.0 && !self.tool.draws_cells() {
            5
        } else {
            1
        };
        for x in (-(half.x as i32)..=half.x as i32).step_by(step) {
            let a = view.screen(vec2(x as f32, -half.y));
            let b = view.screen(vec2(x as f32, half.y));
            draw_line(a.x, a.y, b.x, b.y, 0.5, RULE);
        }
        for y in (-(half.y as i32)..=half.y as i32).step_by(step) {
            let a = view.screen(vec2(-half.x, y as f32));
            let b = view.screen(vec2(half.x, y as f32));
            draw_line(a.x, a.y, b.x, b.y, 0.5, RULE);
        }
        if !self.tool.draws_cells() {
            crate::terrain_view::draw(&terrain, &view);
        }
        if self
            .drag
            .as_ref()
            .is_some_and(|d| matches!(d.kind, DragKind::Draw))
            && let Some(i) = self.selected
        {
            draw_cell_fill(&self.level.items[i], self.item_mesh(i), &view);
        }
        for (i, item) in self.level.items.iter().enumerate() {
            if !matches!(item, LevelItem::Terrain { .. }) || self.selection.contains(&i) {
                draw_item(
                    item,
                    self.item_mesh(i),
                    &view,
                    self.selection.contains(&i),
                    self.vertices && self.selection.len() == 1,
                    self.selection.len() == 1,
                );
            }
        }
        if let Some(drag) = &self.drag
            && matches!(drag.kind, DragKind::Marquee)
        {
            let a = view.screen(drag.start.min(point));
            let b = view.screen(drag.start.max(point));
            draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, Color { a: 0.08, ..CYAN });
            draw_rectangle_lines(a.x, a.y, b.x - a.x, b.y - a.y, 1.5, CYAN);
        }
        if matches!(self.tool, Tool::Erase) && (rect.contains(mouse) || self.drag.is_some()) {
            let start = self.drag.as_ref().map_or(point, |drag| drag.start);
            let (min, max, _) = grid_rectangle(start, point, self.level.size.into(), false);
            let a = view.screen(min);
            let b = view.screen(max);
            draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, Color { a: 0.22, ..PINK });
            draw_rectangle_lines(a.x, a.y, b.x - a.x, b.y - a.y, 2.0, PINK);
        }
        if self.drag.is_none()
            && rect.contains(mouse)
            && !matches!(self.tool, Tool::Select | Tool::Erase)
        {
            if let Some(mut item) = self.placement(point, point)
                && item.fit_inside(self.level.size.into()).is_ok()
            {
                if self.tool.draws_cells() {
                    draw_cell_fill(&item, None, &view);
                }
                draw_item(&item, None, &view, false, false, false);
            }
            draw_circle_lines(mouse.x, mouse.y, 6.0, 1.0, CYAN);
        }
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, RULE);
        text(
            &format!(
                "{} ITEMS / {:.0} x {:.0} M{}",
                self.level.items.len(),
                self.level.size[0],
                self.level.size[1],
                if self.tool.draws_cells() {
                    " / DRAW GRID 1 M"
                } else {
                    ""
                }
            ),
            rect.x + 12.0,
            rect.y + 22.0,
            14.0,
            MUTED,
        );
    }
    fn begin_drag(&mut self, view: &Viewport, mouse: Vec2, point: Vec2) {
        if !matches!(self.tool, Tool::Select) {
            if self.tool.draws_cells() {
                self.drag = Some(Drag {
                    before: self.level.clone(),
                    start: point,
                    kind: if matches!(self.tool, Tool::Erase) {
                        DragKind::Erase
                    } else {
                        DragKind::Draw
                    },
                    selection: self.selection.clone(),
                });
            } else if let Some(item) = self.placement(point, point) {
                let before = self.level.clone();
                self.place(item);
                if !is_key_down(KeyCode::LeftAlt) && !is_key_down(KeyCode::RightAlt) {
                    self.join_touching();
                }
                self.finish_change(before);
            }
            return;
        }
        if let Some(i) = self.selected
            && self.selection.len() == 1
        {
            let item = &self.level.items[i];
            if self.vertices
                && !item.is_circular()
                && matches!(item, LevelItem::Object { .. } | LevelItem::Solid { .. })
            {
                let points = item.world_outline();
                if let Some(vertex) = points
                    .iter()
                    .position(|p| view.screen(v(*p)).distance(mouse) < 11.0)
                {
                    self.drag = Some(Drag {
                        before: self.level.clone(),
                        start: point,
                        kind: DragKind::Vertex(i, vertex),
                        selection: self.selection.clone(),
                    });
                    return;
                }
                if (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift))
                    && let Some(edge) = (0..points.len()).find(|&e| {
                        segment_distance(
                            view.screen(v(points[e])),
                            view.screen(v(points[(e + 1) % points.len()])),
                            mouse,
                        ) < 10.0
                    })
                {
                    let before = self.level.clone();
                    let item = &mut self.level.items[i];
                    let mut outline = item.local_outline();
                    if outline.len() < 128 {
                        let local =
                            sim::Vec2::from_angle(-item.angle()).rotate(s(point) - item.position());
                        outline.insert(edge + 1, local.to_array());
                        if let LevelItem::Object { asset, .. } = item {
                            asset.pinned_edge = None;
                            asset.additional_pinned_edges.clear();
                        }
                        item.reshape(outline);
                        self.finish_change(before);
                    }
                    return;
                }
            } else if !matches!(
                item,
                LevelItem::Spawn { .. }
                    | LevelItem::Flag { .. }
                    | LevelItem::Station { .. }
                    | LevelItem::Pickup { .. }
            ) {
                let corner = resize_corner(item);
                if view.screen(v(corner)).distance(mouse) < 12.0 {
                    self.drag = Some(Drag {
                        before: self.level.clone(),
                        start: point,
                        kind: DragKind::Resize(i),
                        selection: self.selection.clone(),
                    });
                    return;
                }
            }
        }
        let hit = self
            .level
            .items
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, i)| !matches!(i, LevelItem::Terrain { .. }))
            .chain(
                self.level
                    .items
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, i)| matches!(i, LevelItem::Terrain { .. })),
            )
            .find(|(i, item)| {
                self.item_mesh(*i).map_or_else(
                    || contains(&item.world_outline(), s(point)),
                    |mesh| {
                        mesh.contains_point(
                            sim::Vec2::from_angle(-item.angle()).rotate(s(point) - item.position()),
                        )
                    },
                )
            })
            .map(|(i, _)| i);
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let hit =
            hit.filter(|&i| !(shift && matches!(self.level.items[i], LevelItem::Terrain { .. })));
        if let Some(i) = hit {
            if shift {
                if self.selection.contains(&i) {
                    self.selection.retain(|&j| j != i);
                    self.clamp_selection();
                    return;
                }
                self.selection.push(i);
                self.clamp_selection();
            } else if !self.selection.contains(&i) {
                self.select_one(i);
            }
            self.drag = Some(Drag {
                before: self.level.clone(),
                start: point,
                kind: DragKind::Move,
                selection: self.selection.clone(),
            });
        } else {
            if !shift {
                self.clear_selection();
            }
            self.drag = Some(Drag {
                before: self.level.clone(),
                start: point,
                kind: DragKind::Marquee,
                selection: self.selection.clone(),
            });
        }
    }
    fn update_drag(&mut self, point: Vec2) {
        let drag = self.drag.as_ref().unwrap();
        let (start, kind) = (drag.start, drag.kind);
        self.level = drag.before.clone();
        match kind {
            DragKind::Erase => {} // Preview only; apply the cut once on release.
            DragKind::Move => {
                let desired = self.sticky_delta(s(point - start));
                let delta = sim::level::clamp_translation(
                    &self.level.items,
                    &self.selection,
                    self.level.size.into(),
                    desired,
                );
                for &i in &self.selection {
                    self.level.items[i].translate(delta);
                }
            }
            DragKind::Marquee => {
                let lo = s(start.min(point));
                let hi = s(start.max(point));
                let mut selected = self.drag.as_ref().unwrap().selection.clone();
                for (i, item) in self.level.items.iter().enumerate() {
                    if item
                        .world_outline()
                        .iter()
                        .all(|p| p.cmpge(lo).all() && p.cmple(hi).all())
                        && !selected.contains(&i)
                    {
                        selected.push(i);
                    }
                }
                self.selection = selected;
                self.clamp_selection();
            }
            DragKind::Vertex(i, j) => {
                let item = &mut self.level.items[i];
                let mut outline = item.local_outline();
                let half = sim::Vec2::from(self.level.size) * 0.5 - sim::Vec2::splat(0.201);
                outline[j] = sim::Vec2::from_angle(-item.angle())
                    .rotate(s(point).clamp(-half, half) - item.position())
                    .to_array();
                item.reshape(outline);
            }
            DragKind::Resize(i) => {
                let item = &mut self.level.items[i];
                let local = sim::Vec2::from_angle(-item.angle()).rotate(s(point) - item.position());
                let _ = item.resize_inside(
                    (local.abs() * 2.0).max(sim::Vec2::splat(0.3)),
                    self.level.size.into(),
                );
            }
            DragKind::Draw => {
                if let Some(item) = self.placement(start, point) {
                    self.place(item);
                }
            }
        }
    }
    fn place(&mut self, mut item: LevelItem) {
        if let Err(e) = item.fit_inside(self.level.size.into()) {
            self.status = e;
            return;
        }
        // A team has one spawn/flag; placing another moves that marker.
        let existing = self.level.items.iter().position(|old| {
            matches!((&item,old),
            (LevelItem::Spawn {team:a,..},LevelItem::Spawn {team:b,..}) |
            (LevelItem::Flag {team:a,..},LevelItem::Flag {team:b,..}) if a==b)
        });
        let i = existing.unwrap_or(self.level.items.len());
        if i == self.level.items.len() {
            self.level.items.push(item);
        } else {
            self.level.items[i] = item;
        }
        self.select_one(i);
    }
    fn placement(&self, start: Vec2, point: Vec2) -> Option<LevelItem> {
        let (min, max, vertical) = grid_rectangle(
            start,
            point,
            Vec2::from(self.level.size),
            matches!(self.tool, Tool::Bridge),
        );
        let half = Vec2::from(self.level.size) * 0.5 - Vec2::splat(0.201);
        let point = point.clamp(-half, half);
        match &self.tool {
            Tool::Select | Tool::Erase => None,
            Tool::Object(asset) => Some(LevelItem::Object {
                cutouts: vec![],
                asset: asset.clone(),
                position: point.to_array(),
                angle: 0.0,
            }),
            Tool::Wall | Tool::Fence | Tool::SolidWall => {
                if matches!(self.tool, Tool::SolidWall) {
                    return Some(LevelItem::Solid {
                        cutouts: vec![],
                        outline: sim::level::rectangle(s((max - min) * 0.5)),
                        position: ((min + max) * 0.5).to_array(),
                        angle: 0.0,
                    });
                }
                let asset = if matches!(self.tool, Tool::Fence) {
                    sim::level::fence_asset(s(max - min))
                } else {
                    sim::level::wall_asset(s(max - min))
                };
                Some(LevelItem::Object {
                    cutouts: vec![],
                    asset: Box::new(asset),
                    position: ((min + max) * 0.5).to_array(),
                    angle: 0.0,
                })
            }
            Tool::Solid => Some(LevelItem::Solid {
                cutouts: vec![],
                outline: sim::level::rectangle(s((max - min) * 0.5)),
                position: ((min + max) * 0.5).to_array(),
                angle: 0.0,
            }),
            Tool::Terrain(terrain) => Some(LevelItem::Terrain {
                terrain: *terrain,
                min: min.to_array(),
                max: max.to_array(),
            }),
            Tool::Bridge => Some(LevelItem::Bridge {
                min: min.to_array(),
                max: max.to_array(),
                vertical,
            }),
            Tool::Spawn(team) => Some(LevelItem::Spawn {
                team: *team,
                position: point.to_array(),
                angle: if *team == Team::Blue {
                    0.0
                } else {
                    std::f32::consts::PI
                },
            }),
            Tool::Pickup(weapon) => Some(LevelItem::Pickup {
                weapon: *weapon,
                position: point.to_array(),
                respawn_time: 20.0,
            }),
            Tool::Station(station) => Some(LevelItem::Station {
                station: *station,
                position: point.to_array(),
                settings: StationSettings::default(),
            }),
            Tool::Flag(team) => Some(LevelItem::Flag {
                team: *team,
                position: point.to_array(),
            }),
        }
    }
}
/// Fill complete 1 m cells, including both endpoint cells, in either drag direction.
/// Keep the result on the grid even when the pointer leaves the map or a bridge
/// needs additional space for its minimum deck size and projecting booths.
fn grid_rectangle(start: Vec2, point: Vec2, size: Vec2, bridge: bool) -> (Vec2, Vec2, bool) {
    let min = start.floor().min(point.floor());
    let max = start.floor().max(point.floor()) + Vec2::ONE;
    let vertical = (max - min).y > (max - min).x;
    let half = (size * 0.5 - Vec2::splat(0.201)).floor();
    let mut lower = -half;
    let mut upper = half;
    let mut minimum = Vec2::ONE;
    if bridge {
        // Horizontal booths project 1.1 m below the deck; rotating a bridge
        // moves that projection to its left. Reserve two complete grid cells.
        if vertical {
            lower.x += 2.0;
            minimum = vec2(2.0, 3.0);
        } else {
            upper.y -= 2.0;
            minimum = vec2(3.0, 2.0);
        }
    }
    let min = min.clamp(lower, upper - Vec2::ONE);
    let max = max.clamp(lower + Vec2::ONE, upper);
    let limit = if bridge {
        (upper - lower).min(Vec2::splat(190.0))
    } else {
        upper - lower
    };
    let dimensions = (max - min).max(minimum).min(limit);
    let min = min.clamp(lower, upper - dimensions);
    (min, min + dimensions, vertical)
}

fn draw_cell_fill(item: &LevelItem, mesh: Option<&sim::ObjectMesh>, view: &Viewport) {
    if let Some(mesh) = mesh {
        let rotation = sim::Vec2::from_angle(item.angle());
        for triangle in &mesh.triangles {
            let [a, b, c] = triangle
                .map(|i| view.screen(v(item.position() + rotation.rotate(mesh.vertices[i]))));
            draw_triangle(a, b, c, Color { a: 0.12, ..CYAN });
        }
        return;
    }
    let center = v(item.position());
    let half = v(item.dimensions()) * 0.5;
    let a = view.screen(center - half);
    let b = view.screen(center + half);
    draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, Color { a: 0.12, ..CYAN });
    draw_rectangle_lines(a.x, a.y, b.x - a.x, b.y - a.y, 2.0, CYAN);
}

fn contains(points: &[sim::Vec2], p: sim::Vec2) -> bool {
    let mut inside = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
fn segment_distance(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (p - (a + (b - a) * ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0))).length()
}
fn resize_corner(item: &LevelItem) -> sim::Vec2 {
    item.position() + sim::Vec2::from_angle(item.angle()).rotate(item.dimensions() * 0.5)
}
pub(super) fn team_color(team: Team) -> Color {
    if team == Team::Blue { CYAN } else { PINK }
}
fn draw_item(
    item: &LevelItem,
    mesh: Option<&sim::ObjectMesh>,
    view: &Viewport,
    selected: bool,
    vertices: bool,
    handles: bool,
) {
    let color = match item {
        LevelItem::Object { asset, .. } => crate::object_library::asset_color(asset),
        LevelItem::Terrain {
            terrain: TerrainKind::Water,
            ..
        } => Color::from_rgba(91, 171, 227, 255),
        LevelItem::Terrain {
            terrain: TerrainKind::Sand,
            ..
        } => Color::from_rgba(223, 185, 111, 255),
        LevelItem::Terrain {
            terrain: TerrainKind::Elevation,
            ..
        } => Color::from_rgba(190, 161, 120, 255),
        LevelItem::Terrain {
            terrain: TerrainKind::Cliff,
            ..
        } => Color::from_rgba(158, 146, 132, 255),
        LevelItem::Terrain { .. } => MUTED,
        LevelItem::Flag { team, .. } | LevelItem::Spawn { team, .. } => team_color(*team),
        _ => INK,
    };
    let points: Vec<_> = item
        .world_outline()
        .into_iter()
        .map(|p| view.screen(v(p)))
        .collect();
    if let Some(mesh) = mesh {
        let rotation = sim::Vec2::from_angle(item.angle());
        let screen = |p| view.screen(v(item.position() + rotation.rotate(p)));
        for [a, b] in mesh.boundary_segments() {
            let (a, b) = (screen(a), screen(b));
            draw_line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.5 }, color);
        }
    } else {
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            draw_line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.5 }, color);
        }
    }
    if let LevelItem::Object { asset, .. } = item
        && !(asset.surface == sim::Surface::Masonry && asset.components.plasticity.is_none())
    {
        let rotation = sim::Vec2::from_angle(item.angle());
        for [a, b] in asset.support_segments() {
            let a = view.screen(v(item.position() + rotation.rotate(a)));
            let b = view.screen(v(item.position() + rotation.rotate(b)));
            draw_line(a.x, a.y, b.x, b.y, 4.0, CYAN);
        }
    }
    let center = view.screen(v(item.position()));
    match item {
        LevelItem::Pickup {
            weapon,
            respawn_time,
            ..
        } => {
            crate::view::pickup_marker(
                &sim::PickupView {
                    position: item.position(),
                    weapon: *weapon,
                    respawn_time: *respawn_time,
                    remaining: 0.0,
                },
                view,
                1.0,
            );
        }
        LevelItem::Station {
            station, settings, ..
        } => {
            crate::view::station_marker(
                &sim::StationView {
                    kind: *station,
                    position: item.position(),
                    settings: *settings,
                    object: 0,
                    owner: None,
                    capturing: None,
                    progress: 0.0,
                    contested: false,
                },
                view,
                1.0,
                selected || mesh.is_none(),
            );
        }
        LevelItem::Spawn { angle, .. } => {
            let end = center + Vec2::from_angle(*angle) * view.scale * 1.8;
            draw_line(center.x, center.y, end.x, end.y, 2.0, color);
            draw_circle(end.x, end.y, 3.0, color);
        }
        LevelItem::Flag { .. } => {
            crate::view::flag_marker(center, view.scale, color);
        }
        _ => {}
    }
    if !matches!(item, LevelItem::Terrain { .. } | LevelItem::Bridge { .. })
        && (selected || matches!(item, LevelItem::Flag { .. } | LevelItem::Spawn { .. }))
    {
        let width = measure_text(item.label(), None, 14, 1.0).width;
        let x = (center.x - width * 0.5).clamp(view.rect.x + 8.0, view.rect.right() - width - 8.0);
        let y = if matches!(item, LevelItem::Flag { .. }) {
            center.y + 28.0
        } else {
            center.y - 23.0
        };
        text(item.label(), x, y, 14.0, color);
    }
    if selected && handles {
        if vertices
            && !item.is_circular()
            && matches!(item, LevelItem::Object { .. } | LevelItem::Solid { .. })
        {
            for p in points {
                draw_circle(p.x, p.y, 5.0, PAPER);
                draw_circle_lines(p.x, p.y, 5.0, 1.5, CYAN);
            }
        } else if !matches!(
            item,
            LevelItem::Spawn { .. }
                | LevelItem::Flag { .. }
                | LevelItem::Station { .. }
                | LevelItem::Pickup { .. }
        ) {
            let p = view.screen(v(resize_corner(item)));
            draw_rectangle(p.x - 5.0, p.y - 5.0, 10.0, 10.0, CYAN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(tool: Tool, size: [f32; 2]) -> LevelEditor {
        let mut editor = LevelEditor::new();
        editor.level = Level {
            size,
            items: vec![],
            ..Level::default()
        };
        editor.tool = tool;
        editor
    }

    fn checked_placement(editor: &LevelEditor, start: Vec2, end: Vec2) -> LevelItem {
        let mut item = editor.placement(start, end).unwrap();
        let before = item.clone();
        item.fit_inside(editor.level.size.into()).unwrap();
        assert_eq!(
            item, before,
            "boundary fitting must preserve grid alignment"
        );
        let mut level = editor.level.clone();
        level.items.push(item.clone());
        level.validate().unwrap();
        item
    }

    #[test]
    fn drawing_fills_whole_cells_in_both_directions_with_or_without_move_snap() {
        for tool in [
            Tool::Wall,
            Tool::Fence,
            Tool::SolidWall,
            Tool::Solid,
            Tool::Terrain(TerrainKind::Ground),
            Tool::Terrain(TerrainKind::Road),
            Tool::Terrain(TerrainKind::Sand),
            Tool::Terrain(TerrainKind::Elevation),
            Tool::Terrain(TerrainKind::Water),
            Tool::Terrain(TerrainKind::Cliff),
        ] {
            let mut editor = editor(tool, [48.0, 30.0]);
            for snap in [false, true] {
                editor.snap = snap;
                let a = editor.point(vec2(-2.85, 0.15));
                let b = editor.point(vec2(3.95, 2.95));
                let forward = checked_placement(&editor, a, b);
                assert_eq!(forward, checked_placement(&editor, b, a));
                assert_eq!(forward.dimensions(), sim::Vec2::new(7.0, 3.0));
                assert_eq!(forward.position(), sim::Vec2::new(0.5, 1.5));
                assert_eq!(forward.angle(), 0.0);
                let click = editor.point(vec2(-0.01, -1.99));
                let item = checked_placement(&editor, click, click);
                assert_eq!(item.dimensions(), sim::Vec2::ONE);
                assert_eq!(item.position(), sim::Vec2::new(-0.5, -1.5));
            }
        }
    }

    #[test]
    fn drawing_at_map_edges_keeps_complete_cells() {
        for size in [[48.0, 30.0], [13.5, 17.3]] {
            for tool in [
                Tool::Wall,
                Tool::Fence,
                Tool::SolidWall,
                Tool::Solid,
                Tool::Terrain(TerrainKind::Water),
            ] {
                let editor = editor(tool, size);
                for (start, end) in [
                    (Vec2::splat(-500.0), Vec2::splat(500.0)),
                    (Vec2::splat(500.0), Vec2::splat(500.0)),
                    (Vec2::splat(-500.0), Vec2::splat(-500.0)),
                ] {
                    let item = checked_placement(&editor, start, end);
                    assert!(item.world_outline().iter().all(|p| *p == p.floor()));
                }
            }
        }
    }

    #[test]
    fn bridge_grid_accounts_for_minimum_size_booths_and_span_limit() {
        for size in [[12.0, 12.0], [48.0, 30.0], [200.0, 200.0]] {
            let editor = editor(Tool::Bridge, size);
            for (start, end) in [
                (vec2(0.2, 0.2), vec2(0.2, 0.2)),
                (vec2(-500.0, -500.0), vec2(500.0, 400.0)),
                (vec2(-500.0, -500.0), vec2(400.0, 500.0)),
                (vec2(500.0, 500.0), vec2(500.0, 502.0)),
            ] {
                let item = checked_placement(&editor, start, end);
                let LevelItem::Bridge { min, max, vertical } = item else {
                    unreachable!()
                };
                assert!(min.into_iter().chain(max).all(|n| n == n.floor()));
                let dimensions = Vec2::from(max) - Vec2::from(min);
                assert!(dimensions.cmple(Vec2::splat(190.0)).all());
                assert!(
                    dimensions
                        .cmpge(if vertical {
                            vec2(2.0, 3.0)
                        } else {
                            vec2(3.0, 2.0)
                        })
                        .all()
                );
            }
        }
    }
}
