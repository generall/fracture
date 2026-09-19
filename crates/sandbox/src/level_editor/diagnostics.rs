//! Read-only browser authoring diagnostics.
use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
static ACTIVE: AtomicU32 = AtomicU32::new(0);
static ITEMS: AtomicU32 = AtomicU32::new(0);
static SELECTED: AtomicU32 = AtomicU32::new(u32::MAX);
static SELECTION_COUNT: AtomicU32 = AtomicU32::new(0);
static SCALE: AtomicU32 = AtomicU32::new(0);
static ORIGIN_X: AtomicU32 = AtomicU32::new(0);
static ORIGIN_Y: AtomicU32 = AtomicU32::new(0);
pub(super) fn closed() {
    ACTIVE.store(0, Ordering::Relaxed);
}
pub(super) fn publish(editor: &LevelEditor, rect: Rect) {
    ACTIVE.store(1, Ordering::Relaxed);
    ITEMS.store(editor.level.items.len() as u32, Ordering::Relaxed);
    SELECTED.store(
        editor.selected.map_or(u32::MAX, |i| i as u32),
        Ordering::Relaxed,
    );
    SELECTION_COUNT.store(editor.selection.len() as u32, Ordering::Relaxed);
    let view = Viewport::new(rect, editor.navigation.center, editor.navigation.zoom);
    let origin = view.screen(Vec2::ZERO);
    SCALE.store(view.scale.to_bits(), Ordering::Relaxed);
    ORIGIN_X.store(origin.x.to_bits(), Ordering::Relaxed);
    ORIGIN_Y.store(origin.y.to_bits(), Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_active() -> u32 {
    ACTIVE.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_items() -> u32 {
    ITEMS.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_selected() -> u32 {
    SELECTED.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_selection_count() -> u32 {
    SELECTION_COUNT.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_origin_x() -> f32 {
    f32::from_bits(ORIGIN_X.load(Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_origin_y() -> f32 {
    f32::from_bits(ORIGIN_Y.load(Ordering::Relaxed))
}
