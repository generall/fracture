fn main() {
    // quad-snd uses the stable ALSA ABI. A linker script also supports systems
    // with the runtime library installed but no development-package symlink.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(out.join("libasound.so"), "INPUT ( libasound.so.2 )\n").unwrap();
        println!("cargo:rustc-link-search=native={}", out.display());
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        // quad-snd predates explicit wasm_import_module attributes. Import only
        // its known browser API, keeping unresolved application symbols errors.
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let imports = out.join("audio-imports.txt");
        std::fs::write(&imports, "audio_init\naudio_add_buffer\naudio_play_buffer\naudio_source_is_loaded\naudio_source_set_volume\naudio_source_stop\naudio_source_delete\naudio_playback_stop\naudio_playback_set_volume\n").unwrap();
        println!(
            "cargo:rustc-link-arg=--allow-undefined-file={}",
            imports.display()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
}
