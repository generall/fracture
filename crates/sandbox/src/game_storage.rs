//! The match map is stored separately from the sandbox's editor draft.
#[cfg(not(target_arch = "wasm32"))]
pub fn restore() -> Result<Option<String>, String> {
    match std::fs::read_to_string("levels/match-map.json") {
        Ok(json) => Ok(Some(json)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn persist(json: &str) -> Result<(), String> {
    std::fs::create_dir_all("levels").map_err(|e| e.to_string())?;
    std::fs::write("levels/match-map.json.tmp", json).map_err(|e| e.to_string())?;
    std::fs::rename("levels/match-map.json.tmp", "levels/match-map.json").map_err(|e| e.to_string())
}
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn game_map_restore(ptr: *mut u8, capacity: usize) -> i32;
    fn game_map_store(ptr: *const u8, len: usize) -> i32;
}
#[cfg(target_arch = "wasm32")]
pub fn restore() -> Result<Option<String>, String> {
    let mut bytes = vec![0; sim::level::MAX_LEVEL_BYTES];
    let len = unsafe { game_map_restore(bytes.as_mut_ptr(), bytes.len()) };
    if len < 0 {
        return Err("Your saved match map could not be read.".into());
    }
    if len == 0 {
        return Ok(None);
    }
    bytes.truncate(len as usize);
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}
#[cfg(target_arch = "wasm32")]
pub fn persist(json: &str) -> Result<(), String> {
    if unsafe { game_map_store(json.as_ptr(), json.len()) } == 0 {
        Ok(())
    } else {
        Err("Your map could not be saved. Export a JSON copy from the map builder.".into())
    }
}
