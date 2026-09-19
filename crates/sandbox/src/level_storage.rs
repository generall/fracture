//! Small platform boundary for draft persistence and explicit JSON file actions.
#[cfg(not(target_arch = "wasm32"))]
pub fn restore() -> Result<Option<String>, String> {
    match std::fs::read_to_string("levels/draft.json") {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn persist(json: &str) -> Result<(), String> {
    std::fs::create_dir_all("levels").map_err(|e| e.to_string())?;
    std::fs::write("levels/draft.json.tmp", json).map_err(|e| e.to_string())?;
    std::fs::rename("levels/draft.json.tmp", "levels/draft.json").map_err(|e| e.to_string())
}
#[cfg(not(target_arch = "wasm32"))]
pub fn export(json: &str) -> Result<&'static str, String> {
    std::fs::create_dir_all("levels").map_err(|e| e.to_string())?;
    std::fs::write("levels/level.json", json).map_err(|e| e.to_string())?;
    Ok("Exported to levels/level.json")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_import() -> Result<(), String> {
    let path = std::path::Path::new("levels/level.json");
    let size = std::fs::metadata(path)
        .map_err(|e| format!("levels/level.json: {e}"))?
        .len();
    if size > 2_097_152 {
        return Err("Level file exceeds 2 MiB.".into());
    }
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    INCOMING.with(|p| *p.borrow_mut() = Some(Ok(json)));
    Ok(())
}

use std::cell::RefCell;
thread_local! { static INCOMING: RefCell<Option<Result<String,String>>> = const { RefCell::new(None) }; }
pub fn take_import() -> Option<Result<String, String>> {
    INCOMING.with(|p| p.borrow_mut().take())
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn level_store(ptr: *const u8, len: usize) -> i32;
    fn level_restore(ptr: *mut u8, capacity: usize) -> i32;
    fn level_download(ptr: *const u8, len: usize) -> i32;
    fn level_pick_file();
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn level_editor_crate_version() -> u32 {
    1
}
#[cfg(target_arch = "wasm32")]
pub fn restore() -> Result<Option<String>, String> {
    let mut bytes = vec![0; 2_097_152];
    let len = unsafe { level_restore(bytes.as_mut_ptr(), bytes.len()) };
    if len < 0 {
        return Err("Browser draft storage is unavailable or the saved file is too large.".into());
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
    if unsafe { level_store(json.as_ptr(), json.len()) } == 0 {
        Ok(())
    } else {
        Err("Draft could not be saved in this browser. Export a JSON copy.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn export(json: &str) -> Result<&'static str, String> {
    if unsafe { level_download(json.as_ptr(), json.len()) } == 0 {
        Ok("JSON download requested")
    } else {
        Err("Could not download level JSON.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn pick_import() -> Result<(), String> {
    unsafe {
        level_pick_file();
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
thread_local! { static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
// The browser writes only into an allocated, bounded buffer, then queues a copy.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn level_import_buffer(len: usize) -> *mut u8 {
    if len > 2_097_152 {
        return std::ptr::null_mut();
    }
    BUFFER.with(|b| {
        let mut b = b.borrow_mut();
        b.resize(len, 0);
        b.as_mut_ptr()
    })
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn level_import_commit() {
    let result = BUFFER.with(|b| {
        String::from_utf8(std::mem::take(&mut *b.borrow_mut())).map_err(|e| e.to_string())
    });
    INCOMING.with(|p| *p.borrow_mut() = Some(result));
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn level_import_fail(code: u32) {
    let message = if code == 1 {
        "Level file exceeds 2 MiB."
    } else {
        "Could not read level file."
    };
    INCOMING.with(|p| *p.borrow_mut() = Some(Err(message.into())));
}
