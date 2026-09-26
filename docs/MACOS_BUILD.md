# Apple Silicon build

The release target is `aarch64-apple-darwin`, minimum macOS 11.0. Game and sandbox
remain separate executables. `tools/build-macos-arm64.sh` builds both and creates
`target/dist/fracture-macos-arm64.zip` with launchers, credits and a checksum.
All runtime game assets are embedded; the SDK is not included in the archive.

On a Mac with Xcode command-line tools and Rust installed:

```sh
rustup target add aarch64-apple-darwin
bash tools/build-macos-arm64.sh
```

On Linux, supply Clang, llvm-ar, Rust's bundled Mach-O linker and a macOS SDK:

```sh
rustup target add aarch64-apple-darwin
SDKROOT=/path/to/MacOSX11.3.sdk AR_aarch64_apple_darwin=llvm-ar-14 \
  bash tools/build-macos-arm64.sh
```

The initial build used Clang 14, Rust 1.98.1 and the
[macOS 11.3 SDK archive](https://github.com/joseluisq/macosx-sdks/releases/tag/11.3),
verified against published SHA-256
`9adc1373d3879e1973d28ad9f17c9051b02931674a3ec2a2498128989ece2cb1`.
The SDK stays in ignored local tooling. The vendored sound library now uses the
correct `AudioToolbox` framework spelling for case-sensitive cross-linking.

Both binaries are ARM64 Mach-O executables, linked only to macOS system libraries
and frameworks. The linker provides ad-hoc code signatures; this development
archive is not Developer ID signed or notarized. Runtime graphics, audio and LAN
behavior still need a real Apple Silicon Mac; Linux cannot run these binaries.

Extract the archive into a writable folder and double-click `Play.command` or
`Sandbox.command`, or run `./game` / `./sandbox` from Terminal. The launchers use
the extracted folder for saved settings and maps.
