fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_OS");

    // Match the launchers' runtime-only ALSA linking; also supports mixer tests.
    if target.as_deref() == Ok("linux") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        std::fs::write(out.join("libasound.so"), "INPUT ( libasound.so.2 )\n").unwrap();
        println!("cargo:rustc-link-search=native={}", out.display());
    }

    if target.as_deref() == Ok("macos") || target.as_deref() == Ok("ios") {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
    }
}
