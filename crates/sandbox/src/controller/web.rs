use super::*;
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn controller_poll(values: *mut f32) -> u32;
}
pub struct Backend;
impl Backend {
    pub fn new() -> Self {
        Self
    }
    pub fn poll(&mut self) -> Snapshot {
        let mut values = [0.0; 4];
        let state = unsafe { controller_poll(values.as_mut_ptr()) };
        Snapshot {
            connected: state & (1 << 31) != 0,
            buttons: state & 0xffff,
            left: vec2(values[0], values[1]),
            right: vec2(values[2], values[3]),
        }
    }
}
