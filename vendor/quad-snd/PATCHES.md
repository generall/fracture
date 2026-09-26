# Local quad-snd patch

Based on the crates.io `quad-snd` 0.2.8 source, upstream revision
`5043a949ce02bc2119cfb7fb804ebec75ff71447` (Fedor Logachev;
https://github.com/not-fl3/quad-snd), MIT/Apache-2.0 as declared upstream.
The root Cargo.toml selects this copy through `[patch.crates-io]`.

Native mixer fixes:

- Append a loop's wrapped head after its tail in the output buffer. Upstream
  mixed the head at offset zero, overlapping samples and leaving silence.
- Handle exact-boundary endings and loops shorter than one output buffer.
- Smooth starts, volume changes and stops over a 10 ms time constant at the
  mixer's fixed 44.1 kHz rate, avoiding abrupt waveform jumps.
- Limit simultaneous transients to 32 overall / 3 per sound. Retain existing
  voices when the budget is exhausted; continuous loops remain available.
- Soft-limit summed peaks above 0.8 to prevent output clipping.

`build.rs` also supplies the same Linux ALSA runtime linker script as the game
launchers, allowing standalone mixer tests without libasound-dev.

Regression tests: `cargo test -p quad-snd --lib --locked` from the repo root.
Browser playback still uses `web/audio.js`; no new browser imports are added.
