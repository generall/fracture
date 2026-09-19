//! Small platform boundary for draft persistence and explicit JSON file actions.
#[cfg(not(target_arch = "wasm32"))]
pub fn restore() -> Result<Option<String>, String> {
    match std::fs::read_to_string("objects/draft.json") {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn persist(json: &str) -> Result<(), String> {
    std::fs::create_dir_all("objects").map_err(|e| e.to_string())?;
    std::fs::write("objects/draft.json.tmp", json).map_err(|e| e.to_string())?;
    std::fs::rename("objects/draft.json.tmp", "objects/draft.json").map_err(|e| e.to_string())
}
#[cfg(not(target_arch = "wasm32"))]
pub fn export(json: &str) -> Result<&'static str, String> {
    std::fs::create_dir_all("objects").map_err(|e| e.to_string())?;
    std::fs::write("objects/object.json", json).map_err(|e| e.to_string())?;
    Ok("Exported to objects/object.json")
}
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_import() -> Result<(), String> {
    let path = std::path::Path::new("objects/object.json");
    let size = std::fs::metadata(path)
        .map_err(|e| format!("objects/object.json: {e}"))?
        .len();
    if size > 65_536 {
        return Err("Object file exceeds 64 KiB.".into());
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
    fn object_library_store(ptr: *const u8, len: usize) -> i32;
    fn object_library_restore(ptr: *mut u8, capacity: usize) -> i32;
    fn object_store(ptr: *const u8, len: usize) -> i32;
    fn object_restore(ptr: *mut u8, capacity: usize) -> i32;
    fn object_download(ptr: *const u8, len: usize) -> i32;
    fn object_pick_file();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn restore_library() -> Result<Option<String>, String> {
    match std::fs::read_to_string("objects/library.json") {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn persist_library(json: &str) -> Result<(), String> {
    std::fs::create_dir_all("objects").map_err(|e| e.to_string())?;
    std::fs::write("objects/library.json.tmp", json).map_err(|e| e.to_string())?;
    std::fs::rename("objects/library.json.tmp", "objects/library.json").map_err(|e| e.to_string())
}
#[cfg(target_arch = "wasm32")]
pub fn restore_library() -> Result<Option<String>, String> {
    let mut bytes = vec![0; sim::ObjectLibrary::MAX_BYTES];
    let len = unsafe { object_library_restore(bytes.as_mut_ptr(), bytes.len()) };
    if len < 0 {
        return Err("Library storage is unavailable or too large.".into());
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
pub fn persist_library(json: &str) -> Result<(), String> {
    if unsafe { object_library_store(json.as_ptr(), json.len()) } == 0 {
        Ok(())
    } else {
        Err("Library could not be saved. Export a JSON copy of your object.".into())
    }
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn object_editor_crate_version() -> u32 {
    1
}
#[cfg(target_arch = "wasm32")]
pub fn restore() -> Result<Option<String>, String> {
    let mut bytes = vec![0; 65_536];
    let len = unsafe { object_restore(bytes.as_mut_ptr(), bytes.len()) };
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
    if unsafe { object_store(json.as_ptr(), json.len()) } == 0 {
        Ok(())
    } else {
        Err("Draft could not be saved in this browser. Export a JSON copy.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn export(json: &str) -> Result<&'static str, String> {
    if unsafe { object_download(json.as_ptr(), json.len()) } == 0 {
        Ok("JSON download requested")
    } else {
        Err("Could not download object JSON.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn pick_import() -> Result<(), String> {
    unsafe {
        object_pick_file();
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
thread_local! { static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
// The browser writes only into an allocated, bounded buffer, then queues a copy.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn editor_import_buffer(len: usize) -> *mut u8 {
    if len > 65_536 {
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
pub extern "C" fn editor_import_commit() {
    let result = BUFFER.with(|b| {
        String::from_utf8(std::mem::take(&mut *b.borrow_mut())).map_err(|e| e.to_string())
    });
    INCOMING.with(|p| *p.borrow_mut() = Some(result));
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn editor_import_fail(code: u32) {
    let message = if code == 1 {
        "Object file exceeds 64 KiB."
    } else {
        "Could not read object file."
    };
    INCOMING.with(|p| *p.borrow_mut() = Some(Err(message.into())));
}
