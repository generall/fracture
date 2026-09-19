//! Native file selection shared by the level, object and experiment editors.
use std::{
    cell::RefCell,
    io::{Read, Write},
    path::PathBuf,
};

thread_local! {
    static DIRECTORY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn initial_path(directory: &str, filename: &str) -> String {
    let directory = DIRECTORY.with(|d| d.borrow().clone()).unwrap_or_else(|| {
        std::fs::canonicalize(directory)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
    });
    directory.join(filename).to_string_lossy().into_owned()
}

fn remember(path: &std::path::Path) {
    DIRECTORY.with(|d| *d.borrow_mut() = path.parent().map(|p| p.to_path_buf()));
}

pub fn save_json(
    title: &str,
    directory: &str,
    filename: &str,
    json: &str,
) -> Result<&'static str, String> {
    let Some(selected) = tinyfiledialogs::save_file_dialog_with_filter(
        title,
        &initial_path(directory, filename),
        &["*.json"],
        "JSON files",
    ) else {
        return Ok("Save cancelled.");
    };
    let path = PathBuf::from(selected);
    // The dialog handles overwrite confirmation for the exact selected path.
    // Write beside the destination, then replace it only after the full save
    // succeeds. A failed write must not truncate a previously saved map.
    let mut temporary_name = path
        .file_name()
        .ok_or("Choose a file name.")?
        .to_os_string();
    temporary_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let temporary = path.with_file_name(temporary_name);
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, &path)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("{}: {error}", path.display()));
    }
    remember(&path);
    Ok("File saved.")
}

pub fn open_json(title: &str, directory: &str, max_bytes: usize) -> Result<Option<String>, String> {
    let Some(selected) = tinyfiledialogs::open_file_dialog(
        title,
        &initial_path(directory, ""),
        Some((&["*.json"], "JSON files")),
    ) else {
        return Ok(None);
    };
    let path = PathBuf::from(selected);
    let file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Bound the actual read, including a file that grows after the dialog closes.
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "File exceeds the {} KiB size limit.",
            max_bytes / 1024
        ));
    }
    let json = String::from_utf8(bytes).map_err(|e| format!("File is not UTF-8 JSON: {e}"))?;
    remember(&path);
    Ok(Some(json))
}
