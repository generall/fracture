# Current workspace checkpoint — 2026-09-26

Saved at the user's request. Archive: `checkpoints/2026-09-26-current-progress/source.tar.gz`.
The earlier `2026-09-26-game-progress` checkpoint is preserved unchanged.
Git is unavailable: `.git` is empty and read-only, so this is an archive, not a commit.

## Included

All current source, bundled assets, documentation, local maps, object recipes and
audio settings. Completed local gameplay/audio/terrain changes are documented in
[the earlier checkpoint](2026-09-26.md).

The workspace also contains unfinished multiplayer groundwork:

- `world.rs` separates tick setup, player controls and shared physics processing.
- `world/multiplayer.rs` manages up to six tanks with independent controls and weapon
  state in one authoritative simulation, including validated inputs and disconnects.
- Special pickup timers advance once per simulation tick.
- Three focused multiplayer simulation tests pass.

This is not playable LAN multiplayer yet. Transport, discovery/IP joining, lobby,
compressed visibility-filtered replication, client interpolation and six-peer
performance validation remain outstanding; see [the acceptance plan](../LAN_MULTIPLAYER_PLAN.md).

## Validation and resume

Formatting passes. `cargo test --workspace --locked` passes (200 simulation tests,
36 client tests, one ignored simulation test). Full results are recorded alongside the archive in
`workspace-tests.log`. There is an existing-in-this-snapshot unused `drive_tracks`
warning from the in-progress tick refactor. Native/WASM builds and browser checks
have not been rerun after that refactor. Its regression fingerprints have not yet
been compared; the pre-refactor baseline is saved beside the archive.

Resume by comparing the `regression_snapshot` example with that baseline, checking
single-player tick behavior, then continuing the acceptance plan. Do not treat the
previous checkpoint's build/browser results as validation of the unfinished work.

The archive excludes build output, generated WASM, caches, other checkpoints and
environment metadata. Verify with `sha256sum -c SHA256SUMS` from the checkpoint
directory; restore into an empty directory and rebuild from the included Cargo.lock.
