use super::*;
use crate::ui::*;
use sim::VehicleKind;

impl Game {
    pub(crate) fn draw_deployment(&self, size: Vec2, mouse: Vec2, lab: &mut Lab) {
        draw_rectangle(0.0, 0.0, size.x, size.y, Color::new(0.01, 0.02, 0.04, 0.86));
        let width = (size.x - 48.0).min(1050.0);
        let x = (size.x - width) * 0.5;
        let top = (size.y - 380.0) * 0.5;
        text("CHOOSE YOUR VEHICLE", x, top, 30.0, INK);
        let remaining = lab.world.player_respawn_remaining().unwrap_or(0.0);
        let selected = lab
            .world
            .tanks()
            .find(|t| t.id == lab.world.player_tank_id())
            .and_then(|t| t.choice);
        let status = if remaining > 0.0 {
            format!("DEPLOY IN {remaining:.1}s / CHOOSE NOW")
        } else if selected.is_some() {
            "DEPLOYING WHEN SPAWN IS CLEAR".into()
        } else {
            "SELECT TO DEPLOY / THE MATCH CONTINUES".into()
        };
        text(&status, x, top + 32.0, 17.0, MUTED);
        let gap = 16.0;
        let card_width = (width - gap * 2.0) / 3.0;
        for (i, kind) in VehicleKind::ALL.into_iter().enumerate() {
            let left = x + i as f32 * (card_width + gap);
            let y = top + 56.0;
            draw_rectangle(left, y, card_width, 298.0, PAPER);
            draw_rectangle_lines(left, y, card_width, 298.0, 1.0, RULE);
            crate::view::vehicles::draw(
                kind,
                vec2(left + card_width * 0.5, y + 63.0),
                -0.3,
                -0.3,
                30.0,
                CYAN,
                INK,
                [0.0; 2],
            );
            text(kind.label(), left + 16.0, y + 124.0, 21.0, INK);
            let lines = match kind {
                VehicleKind::Scout => [
                    "FAST / LIGHT ARMOR / 360 SIGHT",
                    "SMALL AUTOMATIC GUN",
                    "NO SPECIAL WEAPON PICKUPS",
                ],
                VehicleKind::Tank => [
                    "BALANCED / ARMORED",
                    "ROTATING TURRET / HE + AP",
                    "SPECIAL WEAPON PICKUPS",
                ],
                VehicleKind::Mortar => [
                    "SLOW / HEAVY ARMOR",
                    "ARCING SHELLS / FIXED GUN",
                    "LONG NARROW SIGHT / 6m NEAR",
                ],
            };
            for (j, line) in lines.iter().enumerate() {
                let font = (card_width / 19.0).clamp(12.0, 16.0);
                text(line, left + 16.0, y + 158.0 + j as f32 * 25.0, font, MUTED);
            }
            if button(
                Rect::new(left + 12.0, y + 240.0, card_width - 24.0, 44.0),
                if selected == Some(kind) {
                    "SELECTED"
                } else {
                    "DEPLOY"
                },
                mouse,
                selected == Some(kind),
            ) {
                lab.pending
                    .retain(|c| !matches!(c, sim::Command::Deploy(_)));
                lab.pending.push(sim::Command::Deploy(kind));
            }
        }
    }
}
