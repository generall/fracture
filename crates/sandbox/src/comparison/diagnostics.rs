//! Read-only browser diagnostics for comparison interaction checks.
use super::*;

#[cfg(target_arch = "wasm32")]
mod counters {
    use std::sync::atomic::{AtomicU32, Ordering};
    static ACTIVE: AtomicU32 = AtomicU32::new(0);
    static CASES: AtomicU32 = AtomicU32::new(0);
    static TICK: AtomicU32 = AtomicU32::new(0);
    static CURSOR: AtomicU32 = AtomicU32::new(0);
    static FAVORITE: AtomicU32 = AtomicU32::new(0);
    static SELECTED: AtomicU32 = AtomicU32::new(0);
    static VALID: AtomicU32 = AtomicU32::new(0);
    pub fn publish(c: &super::Comparison) {
        ACTIVE.store(1, Ordering::Relaxed);
        CASES.store(c.experiment.variants.len() as u32, Ordering::Relaxed);
        TICK.store(c.available(), Ordering::Relaxed);
        CURSOR.store(c.state(0).map_or(0, |s| s.frame.tick), Ordering::Relaxed);
        FAVORITE.store(
            c.experiment.favorite.map_or(0, |i| i as u32 + 1),
            Ordering::Relaxed,
        );
        SELECTED.store(c.selected as u32, Ordering::Relaxed);
        VALID.store(c.error.is_none() as u32, Ordering::Relaxed);
    }
    pub fn closed() {
        ACTIVE.store(0, Ordering::Relaxed);
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_active() -> u32 {
        ACTIVE.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_cases() -> u32 {
        CASES.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_tick() -> u32 {
        TICK.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_cursor() -> u32 {
        CURSOR.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_favorite() -> u32 {
        FAVORITE.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_selected() -> u32 {
        SELECTED.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn comparison_valid() -> u32 {
        VALID.load(Ordering::Relaxed)
    }
}
pub(super) fn publish(c: &Comparison) {
    #[cfg(target_arch = "wasm32")]
    counters::publish(c);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = c;
}
pub fn publish_closed() {
    #[cfg(target_arch = "wasm32")]
    counters::closed();
}
