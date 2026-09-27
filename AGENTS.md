# Internal testing

- Keep music, sound effects, and interface sounds silent during internal tests.
- For native game/sandbox tests, launch with `FRACTURE_SILENT=1`. This is a
  process-only override and must not change the player's saved mixer settings.
- Launch isolated test browsers with `--mute-audio`, including audio regression
  suites. Audio assertions should inspect the signal graph without audible output.
- When testing native audio output itself, mute only that application's output
  stream instead of using the master-gain override. Do not mute the entire system.
