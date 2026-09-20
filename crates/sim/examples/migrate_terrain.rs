//! Convert legacy level rectangles without touching authored physical objects.
use sim::level::Level;

fn main() -> Result<(), String> {
    for path in std::env::args().skip(1) {
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut level = Level::from_json(&json)?;
        let count = level.migrate_terrain();
        if count == 0 {
            continue;
        }
        let converted = level.to_json()?;
        if Level::from_json(&converted)? != level {
            return Err("Terrain conversion did not roundtrip.".into());
        }
        std::fs::write(&path, &converted).map_err(|e| e.to_string())?;
        println!(
            "{path}: {count} terrain rectangles → mask; {} objects; {} → {} bytes",
            level.items.len(),
            json.len(),
            converted.len()
        );
    }
    Ok(())
}
