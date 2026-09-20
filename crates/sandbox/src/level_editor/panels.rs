use super::*;
impl LevelEditor {
    pub(super) fn palette_panel(&mut self, size: Vec2, mouse: Vec2) {
        draw_rectangle(0.0, 80.0, 282.0, size.y - 160.0, PAPER);
        text("PLACE ON MAP", 24.0, 115.0, 20.0, INK);
        if button(
            Rect::new(20.0, 133.0, 148.0, 38.0),
            "SELECT / MOVE",
            mouse,
            matches!(self.tool, Tool::Select),
        ) {
            self.commit_field();
            self.select_tool(Tool::Select);
        }
        if button(
            Rect::new(174.0, 133.0, 90.0, 38.0),
            "ERASER",
            mouse,
            matches!(self.tool, Tool::Erase),
        ) {
            self.commit_field();
            self.select_tool(Tool::Erase);
        }
        for (i, label) in ["OBJECTS", "TERRAIN", "SPECIAL"].iter().enumerate() {
            if button(
                Rect::new(20.0 + i as f32 * 83.0, 184.0, 80.0, 36.0),
                label,
                mouse,
                self.category == i,
            ) {
                self.category = i;
                self.page = 0;
            }
        }
        match self.category {
            0 => {
                if button(
                    Rect::new(20.0, 234.0, 118.0, 38.0),
                    "DRAW WALL",
                    mouse,
                    matches!(self.tool, Tool::Wall),
                ) {
                    self.commit_field();
                    self.select_tool(Tool::Wall);
                }
                if button(
                    Rect::new(146.0, 234.0, 118.0, 38.0),
                    "DRAW FENCE",
                    mouse,
                    matches!(self.tool, Tool::Fence),
                ) {
                    self.commit_field();
                    self.select_tool(Tool::Fence);
                }
                if button(
                    Rect::new(20.0, 280.0, 244.0, 38.0),
                    "DRAW UNBREAKABLE WALL",
                    mouse,
                    matches!(self.tool, Tool::SolidWall),
                ) {
                    self.commit_field();
                    self.select_tool(Tool::SolidWall);
                }
                text("LIBRARY / STARTERS + SAVED", 24.0, 346.0, 14.0, MUTED);
                let start = self.page * 6;
                for i in start..(start + 6).min(self.palette.len()) {
                    let a = self.palette[i].clone();
                    if button(
                        Rect::new(20.0, 364.0 + (i - start) as f32 * 46.0, 244.0, 38.0),
                        &a.name,
                        mouse,
                        matches!(&self.tool,Tool::Object(current) if **current==a),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Object(Box::new(a)));
                    }
                }
                if self.palette.len() > 6 {
                    if button(Rect::new(20.0, 660.0, 100.0, 34.0), "PREV", mouse, false) {
                        self.page = self.page.saturating_sub(1);
                    }
                    if button(Rect::new(164.0, 660.0, 100.0, 34.0), "NEXT", mouse, false) {
                        self.page = (self.page + 1).min((self.palette.len() - 1) / 6);
                    }
                }
            }
            1 => {
                for (i, terrain) in [
                    TerrainKind::Ground,
                    TerrainKind::Road,
                    TerrainKind::Sand,
                    TerrainKind::Water,
                    TerrainKind::TallGrass,
                    TerrainKind::Elevation,
                ]
                .into_iter()
                .enumerate()
                {
                    if button(
                        Rect::new(20.0, 238.0 + i as f32 * 48.0, 244.0, 38.0),
                        terrain.label(),
                        mouse,
                        matches!(self.tool,Tool::Terrain(t) if t==terrain),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Terrain(terrain));
                    }
                }
                if button(
                    Rect::new(20.0, 536.0, 244.0, 38.0),
                    "BRIDGE + TWO BOOTHS",
                    mouse,
                    matches!(self.tool, Tool::Bridge),
                ) {
                    self.commit_field();
                    self.select_tool(Tool::Bridge);
                }
                paragraph(
                    "Drag to fill 1 m cells. Elevation extends sight; its edges strongly slow both climbs and descents. Water needs a bridge.",
                    24.0,
                    610.0,
                    240.0,
                    17.0,
                    MUTED,
                );
            }
            _ => {
                if button(
                    Rect::new(20.0, 238.0, 244.0, 38.0),
                    "UNBREAKABLE BLOCK",
                    mouse,
                    matches!(self.tool, Tool::Solid),
                ) {
                    self.commit_field();
                    self.select_tool(Tool::Solid);
                }
                for (i, team) in [Team::Blue, Team::Red].into_iter().enumerate() {
                    if button(
                        Rect::new(20.0, 304.0 + i as f32 * 110.0, 244.0, 38.0),
                        &format!("{} SPAWN", team.label()),
                        mouse,
                        matches!(self.tool,Tool::Spawn(t) if t==team),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Spawn(team));
                    }
                    if button(
                        Rect::new(20.0, 349.0 + i as f32 * 110.0, 244.0, 38.0),
                        &format!("{} FLAG", team.label()),
                        mouse,
                        matches!(self.tool,Tool::Flag(t) if t==team),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Flag(team));
                    }
                }
                for (i, kind) in [StationKind::Repair, StationKind::Radar]
                    .into_iter()
                    .enumerate()
                {
                    if button(
                        Rect::new(20.0, 514.0 + i as f32 * 45.0, 244.0, 38.0),
                        kind.label(),
                        mouse,
                        matches!(self.tool, Tool::Station(k) if k == kind),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Station(kind));
                    }
                }
                for (i, weapon) in [sim::SpecialWeapon::Laser, sim::SpecialWeapon::Rocket]
                    .into_iter()
                    .enumerate()
                {
                    if button(
                        Rect::new(20.0 + i as f32 * 126.0, 604.0, 118.0, 38.0),
                        weapon.label(),
                        mouse,
                        matches!(self.tool, Tool::Pickup(w) if w == weapon),
                    ) {
                        self.commit_field();
                        self.select_tool(Tool::Pickup(weapon));
                    }
                }
                paragraph(
                    "Stations: capture nearby. Pickups: drive over to collect.",
                    24.0,
                    668.0,
                    240.0,
                    17.0,
                    MUTED,
                );
            }
        }
        if button(
            Rect::new(20.0, size.y - 171.0, 244.0, 32.0),
            if self.join {
                "AUTO JOIN / ON"
            } else {
                "AUTO JOIN / OFF"
            },
            mouse,
            self.join,
        ) {
            self.join = !self.join;
        }
        if button(
            Rect::new(20.0, size.y - 126.0, 244.0, 32.0),
            if self.snap {
                "MOVE SNAP 0.25 M / ON"
            } else {
                "MOVE SNAP / OFF"
            },
            mouse,
            self.snap,
        ) {
            self.snap = !self.snap;
        }
    }
    pub(super) fn inspector(&mut self, size: Vec2, mouse: Vec2) {
        let x = size.x - 280.0;
        draw_rectangle(x - 8.0, 80.0, 288.0, size.y - 160.0, PAPER);
        if self.selection.len() > 1 {
            text(
                &format!("{} SELECTED", self.selection.len()),
                x + 4.0,
                115.0,
                20.0,
                CYAN,
            );
            if button(Rect::new(x, 155.0, 122.0, 36.0), "COPY", mouse, false) {
                self.copy_selection();
            }
            if button(
                Rect::new(x + 134.0, 155.0, 122.0, 36.0),
                "PASTE",
                mouse,
                false,
            ) {
                self.paste();
            }
            if button(
                Rect::new(x, 208.0, 256.0, 36.0),
                "MERGE SHAPES / CTRL J",
                mouse,
                false,
            ) {
                self.merge_selection();
            }
            if button(Rect::new(x, 261.0, 122.0, 36.0), "DUPLICATE", mouse, false) {
                self.copy_selection();
                self.paste();
            }
            if button(
                Rect::new(x + 134.0, 261.0, 122.0, 36.0),
                "DELETE",
                mouse,
                false,
            ) {
                self.delete();
            }
            self.field(
                Field::X,
                "MOVE GROUP / X",
                Rect::new(x, 355.0, 122.0, 34.0),
                mouse,
            );
            self.field(
                Field::Y,
                "MOVE GROUP / Y",
                Rect::new(x + 134.0, 355.0, 122.0, 34.0),
                mouse,
            );
            paragraph(
                "Shift-click or drag a box to select. Drag a selected item to move the group.",
                x + 4.0,
                435.0,
                248.0,
                17.0,
                MUTED,
            );
            paragraph(
                "Merge touching rocks or walls with matching properties. Hold Alt to temporarily disable Auto Join.",
                x + 4.0,
                565.0,
                248.0,
                17.0,
                MUTED,
            );
            return;
        }
        if let Some(i) = self.selected {
            let item = self.level.items[i].clone();
            text("SELECTED", x + 4.0, 115.0, 20.0, CYAN);
            text(item.label(), x + 4.0, 145.0, 18.0, INK);
            if !matches!(self.tool, Tool::Select)
                && button(
                    Rect::new(x, 163.0, 256.0, 34.0),
                    "EDIT THIS INSTANCE",
                    mouse,
                    false,
                )
            {
                self.tool = Tool::Select;
            }
            self.field(
                Field::X,
                "X / METRES",
                Rect::new(x, 222.0, 122.0, 34.0),
                mouse,
            );
            self.field(
                Field::Y,
                "Y / METRES",
                Rect::new(x + 134.0, 222.0, 122.0, 34.0),
                mouse,
            );
            if !matches!(
                item,
                LevelItem::Spawn { .. }
                    | LevelItem::Flag { .. }
                    | LevelItem::Station { .. }
                    | LevelItem::Pickup { .. }
            ) {
                self.field(
                    Field::Width,
                    if item.is_circular() {
                        "DIAMETER / M"
                    } else {
                        "WIDTH / M"
                    },
                    Rect::new(x, 294.0, 122.0, 34.0),
                    mouse,
                );
                if !item.is_circular() {
                    self.field(
                        Field::Height,
                        "HEIGHT / M",
                        Rect::new(x + 134.0, 294.0, 122.0, 34.0),
                        mouse,
                    );
                }
            }
            if matches!(
                item,
                LevelItem::Object { .. } | LevelItem::Solid { .. } | LevelItem::Spawn { .. }
            ) {
                self.field(
                    Field::Angle,
                    "ROTATION / DEGREES",
                    Rect::new(x, 366.0, 256.0, 34.0),
                    mouse,
                );
            }
            if !item.is_circular()
                && matches!(item, LevelItem::Object { .. } | LevelItem::Solid { .. })
            {
                if button(
                    Rect::new(x, 422.0, 256.0, 36.0),
                    if self.vertices {
                        "VERTICES / ON"
                    } else {
                        "EDIT OUTLINE VERTICES"
                    },
                    mouse,
                    self.vertices,
                ) {
                    self.commit_field();
                    self.vertices = !self.vertices;
                    self.tool = Tool::Select;
                }
                paragraph(
                    if self.vertices {
                        "Drag a vertex. Shift-click an edge to add a point. New points clear edge supports."
                    } else {
                        "Drag the corner to resize. Large shapes use coarser grains automatically."
                    },
                    x + 4.0,
                    489.0,
                    250.0,
                    16.0,
                    MUTED,
                );
                if let LevelItem::Object { asset, .. } = &item
                    && asset.layout.is_some()
                {
                    self.field(
                        Field::GrainWidth,
                        "GRAIN W / M",
                        Rect::new(x, 599.0, 122.0, 34.0),
                        mouse,
                    );
                    self.field(
                        Field::GrainHeight,
                        "GRAIN H / M",
                        Rect::new(x + 134.0, 599.0, 122.0, 34.0),
                        mouse,
                    );
                }
            }
            if let LevelItem::Pickup { .. } = item {
                self.field(
                    Field::PickupRespawn,
                    "RESPAWN / SECONDS",
                    Rect::new(x, 514.0, 256.0, 34.0),
                    mouse,
                );
                paragraph(
                    "Drive over to collect. One carried special; right-click to use. Starts available.",
                    x,
                    575.0,
                    256.0,
                    16.0,
                    MUTED,
                );
            }
            if let LevelItem::Station { station, .. } = item {
                self.field(
                    Field::CaptureTime,
                    "CAPTURE STAGE / SECONDS",
                    Rect::new(x, 294.0, 256.0, 34.0),
                    mouse,
                );
                self.field(
                    Field::CaptureRadius,
                    "CAPTURE RADIUS / M",
                    Rect::new(x, 366.0, 256.0, 34.0),
                    mouse,
                );
                let (field, label, help) = match station {
                    StationKind::Repair => (
                        Field::RepairRate,
                        "REPAIR / HP PER SECOND",
                        "Repairs living friendly tanks within the capture radius. Enemy stations need one stage to neutralize, then one to capture. Leaving resets the current stage.",
                    ),
                    StationKind::Radar => (
                        Field::RadarRadius,
                        "RADAR RADIUS / M",
                        "Reveals a circle through cover for its owner. Enemy stations need one stage to neutralize, then one to capture. Leaving resets the current stage.",
                    ),
                };
                self.field(field, label, Rect::new(x, 438.0, 256.0, 34.0), mouse);
                paragraph(help, x + 4.0, 515.0, 248.0, 17.0, MUTED);
            }
            if item.is_circular() {
                paragraph(
                    "Circular barrel. Drag the corner or edit its diameter. Its live frame can still deform when crushed.",
                    x + 4.0,
                    440.0,
                    248.0,
                    17.0,
                    MUTED,
                );
            }
            if let LevelItem::Bridge { vertical, .. } = item {
                if button(
                    Rect::new(x, 370.0, 256.0, 36.0),
                    if vertical {
                        "CROSSING / VERTICAL"
                    } else {
                        "CROSSING / HORIZONTAL"
                    },
                    mouse,
                    false,
                ) {
                    let before = self.level.clone();
                    if let LevelItem::Bridge { vertical, .. } = &mut self.level.items[i] {
                        *vertical = !*vertical;
                    }
                    if let Err(e) = self.level.items[i].fit_inside(self.level.size.into()) {
                        self.status = e;
                    }
                    self.finish_change(before);
                }
                paragraph(
                    "The rectangle is the deck. Both booths need dry banks. Shoot or ram a booth to collapse the crossing.",
                    x + 4.0,
                    445.0,
                    250.0,
                    17.0,
                    MUTED,
                );
            }
            if matches!(item, LevelItem::Flag { .. } | LevelItem::Spawn { .. }) {
                paragraph(
                    "One marker of each kind per team. A spawn sets the tank position and heading when testing that team.",
                    x + 4.0,
                    445.0,
                    250.0,
                    17.0,
                    MUTED,
                );
            }
            if let LevelItem::Object { asset, .. } = &item {
                let blocks = asset.blocks_vision();
                if button(
                    Rect::new(x, 658.0, 256.0, 34.0),
                    if blocks {
                        "SIGHT / BLOCKS"
                    } else {
                        "SIGHT / SEE THROUGH"
                    },
                    mouse,
                    blocks,
                ) {
                    self.commit_field();
                    let before = self.level.clone();
                    if let LevelItem::Object { asset, .. } = &mut self.level.items[i] {
                        asset.components.blocks_vision = Some(!blocks);
                    }
                    self.finish_change(before);
                }
            }
            if button(
                Rect::new(x, size.y - 171.0, 122.0, 36.0),
                "DUPLICATE",
                mouse,
                false,
            ) {
                self.commit_field();
                self.copy_selection();
                self.paste();
            }
            if button(
                Rect::new(x + 134.0, size.y - 171.0, 122.0, 36.0),
                "DELETE",
                mouse,
                false,
            ) {
                self.commit_field();
                self.delete();
            }
            if button(
                Rect::new(x, size.y - 126.0, 256.0, 32.0),
                "MAP SETTINGS",
                mouse,
                false,
            ) {
                self.commit_field();
                self.clear_selection();
                self.tool = Tool::Select;
            }
        } else {
            text("MAP", x + 4.0, 115.0, 20.0, CYAN);
            self.field(
                Field::Name,
                "LEVEL NAME",
                Rect::new(x, 163.0, 256.0, 36.0),
                mouse,
            );
            self.field(
                Field::MapWidth,
                "WIDTH / M",
                Rect::new(x, 244.0, 122.0, 34.0),
                mouse,
            );
            self.field(
                Field::MapHeight,
                "HEIGHT / M",
                Rect::new(x + 134.0, 244.0, 122.0, 34.0),
                mouse,
            );
            text("ENEMY ACTIVITY", x + 4.0, 316.0, 17.0, INK);
            for (i, activity) in sim::level::EnemyActivity::ALL.into_iter().enumerate() {
                if button(
                    Rect::new(x, 334.0 + i as f32 * 43.0, 256.0, 36.0),
                    activity.label(),
                    mouse,
                    self.level.enemy_activity == activity,
                ) {
                    self.commit_field();
                    let before = self.level.clone();
                    self.level.enemy_activity = activity;
                    self.finish_change(before);
                }
            }
            paragraph(
                self.level.enemy_activity.description(),
                x + 4.0,
                535.0,
                248.0,
                16.0,
                MUTED,
            );
            paragraph(
                "The opposing team spawn places the enemy. Both tanks respawn after 3 seconds. Choose your team above, then Play Level.",
                x + 4.0,
                625.0,
                248.0,
                16.0,
                MUTED,
            );
        }
    }
}

// Sidebar help can use more than the two lines used by compact library cards.
fn paragraph(value: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    let mut line = String::new();
    let mut row = 0;
    for word in value.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && measure_text(&candidate, None, size as u16, 1.0).width > width {
            text(&line, x, y + row as f32 * 22.0, size, color);
            row += 1;
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        text(&line, x, y + row as f32 * 22.0, size, color);
    }
}
