//! Small platform boundary for experiment-library persistence and explicit JSON file actions.
#[cfg(not(target_arch = "wasm32"))]
pub fn restore() -> Result<Option<String>, String> {
    match std::fs::read_to_string("objects/experiments.json") {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn persist(json: &str) -> Result<(), String> {
    std::fs::create_dir_all("objects").map_err(|e| e.to_string())?;
    std::fs::write("objects/experiments.json.tmp", json).map_err(|e| e.to_string())?;
    std::fs::rename("objects/experiments.json.tmp", "objects/experiments.json")
        .map_err(|e| e.to_string())
}
#[cfg(not(target_arch = "wasm32"))]
pub fn export(json: &str) -> Result<&'static str, String> {
    crate::file_dialog::save_json("Save experiment as", "objects", "experiment.json", json)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_import() -> Result<(), String> {
    if let Some(json) = crate::file_dialog::open_json("Open experiment", "objects", 8388608)? {
        INCOMING.with(|p| *p.borrow_mut() = Some(Ok(json)));
    }
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
    fn experiment_start_barrels() -> i32;
    fn experiment_store(ptr: *const u8, len: usize) -> i32;
    fn experiment_restore(ptr: *mut u8, capacity: usize) -> i32;
    fn experiment_download(ptr: *const u8, len: usize) -> i32;
    fn experiment_pick_file();
}
#[cfg(target_arch = "wasm32")]
pub fn startup() -> i32 {
    // Keep the existing browser import name for compatibility with old builds.
    unsafe { experiment_start_barrels() }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn startup() -> i32 {
    std::env::args()
        .find_map(|s| match s.as_str() {
            "--barrels" => Some(1),
            "--crush" => Some(2),
            "--barrel-shot" => Some(3),
            "--barrel-shot-free" => Some(4),
            "--barrel-release" => Some(5),
            _ => None,
        })
        .unwrap_or(0)
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn experiment_range_crate_version() -> u32 {
    1
}
#[cfg(target_arch = "wasm32")]
pub fn restore() -> Result<Option<String>, String> {
    let mut bytes = vec![0; 8_388_608];
    let len = unsafe { experiment_restore(bytes.as_mut_ptr(), bytes.len()) };
    if len < 0 {
        return Err(
            "Browser experiment storage is unavailable or the saved file is too large.".into(),
        );
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
    if unsafe { experiment_store(json.as_ptr(), json.len()) } == 0 {
        Ok(())
    } else {
        Err("Experiment could not be saved in this browser. Export a JSON copy.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn export(json: &str) -> Result<&'static str, String> {
    if unsafe { experiment_download(json.as_ptr(), json.len()) } == 0 {
        Ok("JSON download requested")
    } else {
        Err("Could not download experiment JSON.".into())
    }
}
#[cfg(target_arch = "wasm32")]
pub fn pick_import() -> Result<(), String> {
    unsafe {
        experiment_pick_file();
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
thread_local! { static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
// The browser writes only into an allocated, bounded buffer, then queues a copy.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn experiment_import_buffer(len: usize) -> *mut u8 {
    if len > 8_388_608 {
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
pub extern "C" fn experiment_import_commit() {
    let result = BUFFER.with(|b| {
        String::from_utf8(std::mem::take(&mut *b.borrow_mut())).map_err(|e| e.to_string())
    });
    INCOMING.with(|p| *p.borrow_mut() = Some(result));
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn experiment_import_fail(code: u32) {
    let message = if code == 1 {
        "Experiment file exceeds 8 MiB."
    } else {
        "Could not read experiment file."
    };
    INCOMING.with(|p| *p.borrow_mut() = Some(Err(message.into())));
}
