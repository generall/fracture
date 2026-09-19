//! Read-only browser diagnostics for editor interaction checks.
#[cfg(target_arch = "wasm32")]
use super::*;

#[cfg(target_arch = "wasm32")]
mod counters {
    use std::sync::atomic::{AtomicU32, Ordering};
    pub static TAB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    pub static MENU: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    pub static ACTIVE: AtomicU32 = AtomicU32::new(0);
    pub static VERTICES: AtomicU32 = AtomicU32::new(0);
    pub static CELLS: AtomicU32 = AtomicU32::new(0);
    pub static LIBRARY: AtomicU32 = AtomicU32::new(0);
    pub static SAVED: AtomicU32 = AtomicU32::new(0);
    #[unsafe(no_mangle)]
    pub extern "C" fn editor_library_open() -> u32 {
        LIBRARY.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn editor_library_saved() -> u32 {
        SAVED.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn editor_active() -> u32 {
        ACTIVE.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn editor_vertices() -> u32 {
        VERTICES.load(Ordering::Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn editor_cells() -> u32 {
        CELLS.load(Ordering::Relaxed)
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) fn publish(editor: &Editor) {
    use std::sync::atomic::Ordering;
    counters::LIBRARY.store(u32::from(editor.library.open), Ordering::Relaxed);
    counters::SAVED.store(editor.library.len() as u32, Ordering::Relaxed);
    counters::TAB.store(
        match editor.tab {
            Tab::Shape => 0,
            Tab::Fracture => 1,
            Tab::Material => 2,
        },
        Ordering::Relaxed,
    );
    counters::MENU.store(
        match editor.menu {
            None => 0,
            Some(Menu::File) => 1,
            Some(Menu::View) => 2,
        },
        Ordering::Relaxed,
    );
    counters::ACTIVE.store(1, Ordering::Relaxed);
    counters::VERTICES.store(editor.asset.outline.len() as u32, Ordering::Relaxed);
    counters::CELLS.store(
        editor.mesh.as_ref().map_or(0, |m| m.triangles.len()) as u32,
        Ordering::Relaxed,
    );
}
pub fn publish_closed() {
    #[cfg(target_arch = "wasm32")]
    counters::ACTIVE.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn editor_tab() -> u32 {
    counters::TAB.load(std::sync::atomic::Ordering::Relaxed)
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn editor_menu() -> u32 {
    counters::MENU.load(std::sync::atomic::Ordering::Relaxed)
}
