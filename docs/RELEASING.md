# Local builds and releases

Download ready-to-run packages from [the latest release](https://github.com/generall/fracture/releases/latest).
The repository's existing visibility also applies to its releases.

| Package | Installation |
| --- | --- |
| `fracture-VERSION-linux-x86_64.tar.gz` | Extract, run `./fracture.sh`. Run `./install.sh` for a per-user install and application menu entry. |
| `fracture-VERSION-steamdeck-x86_64.tar.gz` | Extract in Desktop Mode and run `./install.sh`. Add the installed `fracture-deck.sh` as a non-Steam game, choose Steam Input's Gamepad template, and leave Proton disabled. Launch from Gaming Mode. |
| `fracture-VERSION-macos-arm64.zip` | Apple Silicon: extract, drag `Fracture.app` to Applications and open it. |
| `fracture-VERSION-macos-x86_64.zip` | Intel Mac: extract, drag `Fracture.app` to Applications and open it. |

Each package includes the game, a separate sandbox/editor, bundled assets and SDL2
controller support, upstream license notices, build provenance and a README.
Each archive has a SHA-256 sidecar. Linux requires glibc 2.28+, X11/XWayland,
OpenGL, libX11, libXi, libxkbcommon, ALSA (`libasound.so.2`) and `libgcc_s.so.1`. The build uses PyPA's
manylinux 2.28 environment; the downloads are native archives, not Python wheels.
macOS requires 11.0+. macOS app bundles built on Linux are inspected but cannot
be runtime-tested there. They are not Developer ID signed or notarized. After
attempting to open an app, use **System Settings → Privacy & Security → Open Anyway**
if macOS blocks it; see [Apple's instructions](https://support.apple.com/102445).

Linux saves use `$XDG_DATA_HOME/fracture`, defaulting to `~/.local/share/fracture`.
macOS saves use `~/Library/Application Support/Fracture`. `FRACTURE_DATA_DIR`
overrides either location. Updates replace the installation and preserve saves.
Older macOS `.command` packages saved inside their extracted folder: copy the
old `objects/`, `levels/`, `checkpoints/` and `audio-settings.txt` into the new
save directory if you want to keep them.

## Build everything on Linux

No GitHub Actions, hosted macOS runner, Steamworks account, Python packaging
registry or signing credentials are required. GitHub CLI authentication is
needed only for publishing to this repository's GitHub Releases.

Prerequisites:

- x86_64 Linux, Docker access, Git, curl, tar and Python 3.12+.
- Rustup/Cargo on the host. The release script installs the pinned compiler,
  rustfmt, Clippy and cross-target standard libraries into the ignored
  `target/release-tools/host-rustup` directory.
- Clang, LLVM's `llvm-ar-14`, and `7z` for macOS cross-builds. Override the compiler
  and archiver with `FRACTURE_MACOS_CLANG` and `FRACTURE_MACOS_AR` if needed.
- An extracted macOS SDK, supplied separately. Point `SDKROOT` at its absolute
  path. The existing `.tools/apple-arm/MacOSX11.3.sdk` is detected automatically
  on this development machine. The SDK is never included in packages.

```sh
# Commit the source you intend to release first.
SDKROOT=/absolute/path/to/MacOSX11.3.sdk bash tools/build-release.sh
```

The script runs format checks, workspace tests and Clippy; generates third-party
notices; builds both Linux packages inside the pinned manylinux image; then
cross-compiles Apple Silicon and Intel macOS apps locally. Rust uses locked
dependencies. SDL source and the official universal macOS controller framework
are checksum-verified. Linux builds SDL2's controller runtime from source;
macOS packages contain the matching architecture from SDL's official framework.
`tools/release-config.sh` records the compiler, container digest and SDL inputs.

Linux binaries are checked for architecture, unbundled dependencies and a maximum
glibc requirement of 2.28. The actual archives are extracted, installed twice and
launched under Xvfb in the container. These checks cover the game, sandbox, Deck
launcher, bundled controller library, paths containing spaces and save preservation.
Tests use `FRACTURE_SILENT=1` and a process-local null ALSA device, without changing
the player's saved mixer or system output. macOS binaries are checked for the
requested architecture, system-only library dependencies and deployment target.

All output stays in `target/`. Release archives and checksums are in `target/dist/`.
Builds are incremental; downloaded compilers and SDL inputs are cached. Set
`FRACTURE_BUILD_JOBS` to control compiler parallelism (default 4). Building one
platform while iterating is also supported:

```sh
bash tools/build-release.sh linux            # Linux and Deck, plus launch checks
bash tools/build-release.sh macos-arm64
bash tools/build-release.sh macos-x86_64
```

On a Mac, use the appropriate `macos-*` command with Xcode command-line tools
installed. The same packaging path produces app bundles. `tools/build-macos-arm64.sh`
remains a compatibility entry point for the lower-level Apple Silicon builder;
the `build-release.sh` commands also install prerequisites and run checks.

## Publish this version or the next one

1. Update `[workspace.package].version` in `Cargo.toml` and refresh the workspace
   package versions in `Cargo.lock` with `cargo check --workspace`. Use a new
   three-component version such as `0.2.0`.
2. Commit and push the intended source, including release script changes.
3. Run `bash tools/build-release.sh` as above from that clean commit.
4. Publish the packages:

```sh
bash tools/publish-release.sh v0.2.0
```

The publishing script requires clean tracked files, a tag matching the workspace version,
and all four packages identifying the current commit. It creates a source archive,
verifies archive checksums, layouts, executable permissions and provenance, pushes
an annotated tag, creates a draft release, uploads the five archives and their
checksums, and publishes only after every upload succeeds. It never starts a
GitHub Actions workflow. A failed draft upload can be retried with the same command;
already published releases cannot be overwritten. Existing repository visibility
is preserved. Unrelated untracked files are excluded from the source archive.

The launch checks establish rendering and startup in the manylinux environment;
macOS graphics, audio and LAN behavior need a real Mac. Physical Steam Deck controls,
Gaming Mode, overlay, sleep/wake and dock/undock still need device checks. This
workflow publishes downloads to GitHub, not a Steam store depot, and does not
claim Steam Deck Verified status.
