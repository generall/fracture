# macOS packages

Download Apple Silicon (`macos-arm64`) or Intel (`macos-x86_64`) packages from
[the latest release](https://github.com/generall/fracture/releases/latest).
Both require macOS 11.0+. Extract the archive, drag `Fracture.app` to Applications,
and double-click it. `Fracture Sandbox.app` provides the development sandbox
and editors. SDL2 controller support, game assets and license notices are bundled.

Saves live in `~/Library/Application Support/Fracture`; `FRACTURE_DATA_DIR`
overrides this. Replacing the apps preserves saves. Older `.command` packages
saved inside their extracted directory; see [the migration instructions](RELEASING.md).

These development packages are not Developer ID signed or notarized. If blocked
after attempting to open an app, use **System Settings → Privacy & Security →
Open Anyway**. See [Apple's instructions](https://support.apple.com/102445).
Linux cross-builds are checked for architecture, system library dependencies
and minimum OS version; runtime graphics, audio and LAN behavior require a Mac.

## Build locally

On a Mac with Xcode command-line tools, Rustup, curl and Python 3.12+ installed:

```sh
bash tools/build-release.sh macos-arm64
# Or, for an Intel package:
bash tools/build-release.sh macos-x86_64
```

On x86_64 Linux, also install Clang, LLVM's `llvm-ar-14` and `7z`, then supply
an extracted macOS SDK:

```sh
SDKROOT=/absolute/path/to/MacOSX11.3.sdk bash tools/build-release.sh macos-arm64
SDKROOT=/absolute/path/to/MacOSX11.3.sdk bash tools/build-release.sh macos-x86_64
```

`tools/build-macos-arm64.sh` remains a compatibility entry point for the
lower-level builder. The full commands above install the pinned compiler and
cross-target standard libraries, collect notices and run the Rust checks.
All packages and checksums are written to `target/dist/`.

The initial SDK was the [macOS 11.3 SDK archive](https://github.com/joseluisq/macosx-sdks/releases/tag/11.3),
verified against SHA-256 `9adc1373d3879e1973d28ad9f17c9051b02931674a3ec2a2498128989ece2cb1`.
The SDK stays in ignored local tooling and is not distributed. The official
SDL2 universal framework is downloaded separately and checksum-verified before
the required architecture is packaged.

See [the release guide](RELEASING.md) to build all platforms and publish the
next version entirely from a local machine, without GitHub Actions.
