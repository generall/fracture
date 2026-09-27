//! Directional focus shared by immediate-mode menus, with a modal text keyboard.
use crate::{
    controller::{self, Actions},
    ui,
};
use macroquad::prelude::*;
use std::cell::RefCell;

#[derive(Clone)]
struct Node {
    rect: Rect,
    preferred: bool,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    label: String,
}
struct Keyboard {
    id: usize,
    value: String,
    max: usize,
    shift: bool,
}
#[derive(Default)]
struct State {
    context: u64,
    active: bool,
    enabled: bool,
    actions: Actions,
    nodes: Vec<Node>,
    focus: usize,
    confirm: bool,
    keyboard: Option<Keyboard>,
    keyboard_just_opened: bool,
    keyboard_layer: bool,
    result: Option<(usize, String)>,
}
thread_local! { static UI: RefCell<State> = RefCell::new(State::default()); }

pub fn begin(context: u64, active: bool, enabled: bool, actions: Actions) {
    UI.with_borrow_mut(|s| {
        let context = if s.keyboard.is_some() {
            u64::MAX
        } else {
            context
        };
        let previous = std::mem::take(&mut s.nodes);
        if s.context != context {
            s.focus = 0;
            s.confirm = false;
        } else {
            s.focus = s.focus.min(previous.len().saturating_sub(1));
            if actions.navigation != Vec2::ZERO {
                s.focus = neighbor(&previous, s.focus, actions.navigation);
            }
            s.confirm = actions.pressed(controller::CONFIRM);
        }
        // On first activation, prefer the primary action on the current screen.
        if active && !s.active && !previous.is_empty() {
            s.focus = previous.iter().position(|n| n.preferred).unwrap_or(0);
        }
        s.context = context;
        s.active = active;
        s.enabled = enabled;
        s.actions = actions;
        s.keyboard_layer = false;
        s.keyboard_just_opened = false;
    });
}
fn neighbor(nodes: &[Node], current: usize, direction: Vec2) -> usize {
    let Some(origin) = nodes.get(current) else {
        return 0;
    };
    let center = |r: Rect| vec2(r.x + r.w * 0.5, r.y + r.h * 0.5);
    let direction = direction.normalize_or_zero();
    nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let delta = center(n.rect) - center(origin.rect);
            let forward = delta.dot(direction);
            let sideways = delta.perp_dot(direction).abs();
            (forward > 1.0).then_some((i, forward + sideways * 3.0))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(current, |(i, _)| i)
}
pub fn interact(rect: Rect, label: &str, preferred: bool) -> (bool, bool) {
    UI.with_borrow_mut(|s| {
        if !s.enabled || (s.keyboard.is_some() && !s.keyboard_layer) {
            return (false, false);
        }
        let index = s.nodes.len();
        s.nodes.push(Node {
            rect,
            preferred,
            label: label.to_owned(),
        });
        let focused = s.active && index == s.focus;
        let clicked = focused && s.confirm;
        if clicked {
            s.confirm = false;
        }
        (focused, clicked)
    })
}
pub fn active() -> bool {
    UI.with_borrow(|s| s.active)
}
pub fn keyboard_open() -> bool {
    UI.with_borrow(|s| s.keyboard.is_some())
}
pub fn pointer_allowed() -> bool {
    UI.with_borrow(|s| !s.keyboard_just_opened && (s.keyboard.is_none() || s.keyboard_layer))
}
pub fn back() -> bool {
    !keyboard_open()
        && (is_key_pressed(KeyCode::Escape)
            || UI.with_borrow(|s| s.actions.pressed(controller::BACK)))
}
pub fn prompt<'a>(keyboard: &'a str, pad: &'a str) -> &'a str {
    if active() { pad } else { keyboard }
}
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn edit_text(id: usize, value: &str, max: usize) {
    UI.with_borrow_mut(|s| {
        s.keyboard = Some(Keyboard {
            id,
            value: value.into(),
            max,
            shift: false,
        });
        s.keyboard_just_opened = true;
        s.confirm = false;
    });
}
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn take_text(id: usize) -> Option<String> {
    UI.with_borrow_mut(|s| {
        if s.result.as_ref().is_some_and(|(key, _)| *key == id) {
            s.result.take().map(|(_, value)| value)
        } else {
            None
        }
    })
}
pub fn draw_keyboard(size: Vec2, mouse: Vec2) {
    let Some(mut keyboard) = UI.with_borrow_mut(|s| {
        s.keyboard_layer = true;
        // Underlying controls were blocked; only keyboard nodes get focus.
        s.keyboard.as_ref().map(|k| Keyboard {
            id: k.id,
            value: k.value.clone(),
            max: k.max,
            shift: k.shift,
        })
    }) else {
        return;
    };
    let cancel =
        is_key_pressed(KeyCode::Escape) || UI.with_borrow(|s| s.actions.pressed(controller::BACK));
    draw_rectangle(0.0, 0.0, size.x, size.y, ui::PAPER);
    let x = (size.x - 800.0) * 0.5;
    let y = (size.y - 460.0) * 0.5;
    ui::text("ENTER TEXT", x, y + 24.0, 28.0, ui::INK);
    ui::field_text(
        &keyboard.value,
        Rect::new(x, y + 45.0, 800.0, 50.0),
        32.0,
        24.0,
        true,
    );
    while let Some(c) = get_char_pressed() {
        if !c.is_control() && keyboard.value.len() + c.len_utf8() <= keyboard.max {
            keyboard.value.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        keyboard.value.pop();
    }
    for (row, letters) in ["1234567890", "qwertyuiop", "asdfghjkl-", "zxcvbnm.:_"]
        .into_iter()
        .enumerate()
    {
        for (col, c) in letters.chars().enumerate() {
            let c = if keyboard.shift {
                c.to_ascii_uppercase()
            } else {
                c
            };
            if ui::button(
                Rect::new(
                    x + col as f32 * 80.0,
                    y + 110.0 + row as f32 * 58.0,
                    72.0,
                    48.0,
                ),
                &c.to_string(),
                mouse,
                false,
            ) && keyboard.value.len() + c.len_utf8() <= keyboard.max
            {
                keyboard.value.push(c);
            }
        }
    }
    for (i, label) in ["SHIFT", "SPACE", "DELETE", "DONE", "CANCEL"]
        .into_iter()
        .enumerate()
    {
        if ui::button(
            Rect::new(x + i as f32 * 160.0, y + 350.0, 152.0, 48.0),
            label,
            mouse,
            false,
        ) {
            match i {
                0 => keyboard.shift = !keyboard.shift,
                1 if keyboard.value.len() < keyboard.max => keyboard.value.push(' '),
                2 => {
                    keyboard.value.pop();
                }
                3 => {
                    UI.with_borrow_mut(|s| {
                        s.result = Some((keyboard.id, keyboard.value.clone()));
                        s.keyboard = None;
                    });
                    return;
                }
                4 => {
                    UI.with_borrow_mut(|s| s.keyboard = None);
                    return;
                }
                _ => {}
            }
        }
    }
    ui::text(
        "D-PAD / SELECT     A / TYPE     B / CANCEL",
        x,
        y + 435.0,
        20.0,
        ui::MUTED,
    );
    UI.with_borrow_mut(|s| s.keyboard = if cancel { None } else { Some(keyboard) });
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn controller_active() -> u32 {
    active() as u32
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn controller_focus_x() -> f32 {
    UI.with_borrow(|s| s.nodes.get(s.focus).map_or(-1.0, |n| n.rect.x))
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn controller_focus_y() -> f32 {
    UI.with_borrow(|s| s.nodes.get(s.focus).map_or(-1.0, |n| n.rect.y))
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn controller_focus_label() -> u32 {
    UI.with_borrow(|s| {
        s.nodes.get(s.focus).map_or(0, |n| {
            n.label.bytes().fold(2166136261u32, |hash, byte| {
                (hash ^ u32::from(byte)).wrapping_mul(16777619)
            })
        })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modal_keyboard_blocks_underlying_actions_and_context_changes_consume_confirm() {
        UI.with_borrow_mut(|s| *s = State::default());
        let accept = Actions {
            pressed: controller::CONFIRM,
            ..Actions::default()
        };
        let rect = Rect::new(0., 0., 100., 40.);
        begin(1, true, true, accept);
        assert!(!interact(rect, "OPEN", false).1);
        begin(1, true, true, accept);
        assert!(interact(rect, "OPEN", false).1);
        edit_text(7, "Pilot", 32);
        assert!(!pointer_allowed());
        assert!(!interact(rect, "HOST", false).1);
        UI.with_borrow_mut(|s| s.keyboard_layer = true);
        assert!(!pointer_allowed(), "opening tap cannot also type a key");
        begin(1, true, true, accept);
        UI.with_borrow_mut(|s| s.keyboard_layer = true);
        assert!(pointer_allowed());
        assert!(!interact(rect, "q", false).1, "opening A cannot also type");
        begin(1, true, true, accept);
        UI.with_borrow_mut(|s| s.keyboard_layer = true);
        assert!(interact(rect, "q", false).1);
        UI.with_borrow_mut(|s| {
            s.keyboard = None;
            s.result = Some((7, "Pilotq".into()));
        });
        assert_eq!(take_text(0), None);
        assert_eq!(take_text(7).as_deref(), Some("Pilotq"));
        assert_eq!(take_text(7), None);
        begin(1, true, true, accept);
        assert!(!interact(rect, "HOST", false).1, "closing A cannot host");
    }
    #[test]
    fn spatial_focus_handles_rows_and_missing_neighbors() {
        let nodes: Vec<_> = [(0., 0.), (100., 0.), (0., 100.)]
            .into_iter()
            .map(|(x, y)| Node {
                rect: Rect::new(x, y, 80., 40.),
                preferred: false,
                label: String::new(),
            })
            .collect();
        assert_eq!(neighbor(&nodes, 0, Vec2::X), 1);
        assert_eq!(neighbor(&nodes, 0, Vec2::Y), 2);
        assert_eq!(neighbor(&nodes, 0, -Vec2::X), 0);
    }
}
