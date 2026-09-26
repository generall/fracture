# Sound design

Both applications include an original orchestral military soundtrack and 30 effects/loops. The score is rendered with sampled instruments; the effects are synthesized. All finished assets are bundled into the executables and WebAssembly and play offline.

## Music

- **March:** 88 BPM, a restrained WWII-style military theme in D minor opening into F major. Sampled French horns, low trombones, bowed strings, bass drum, timpani and field-snare grace strokes. Longer phrases replace the former bright synth melody; there are no high bell runs. A 16-bar, 43.6-second loop.
- **Battle:** 132 BPM, heavier brass doubling, low cello ostinatos, timpani and snare rolls. A 16-bar, 29.1-second loop.
- A living enemy's **live visible silhouette** activates battle music. Fog memories do not count; foliage, cover and radar use the existing visibility rules. The fade into battle takes two seconds. After contact disappears there is a four-second hold, followed by a three-second return to the march. The renderer's short visibility grace period still applies.
- Both arrangements keep running underneath the mix. Equal-power crossfades preserve level without restarting tracks. Menus use the march; pause keeps the current blend at a quieter level. Results return to the march.

## Effects

HE has a deep cannon report; AP has a shorter metallic crack; rockets have an ignition roar. The laser uses a sustained electrical hum. Plasma has an ascending charge sound followed by a heavy electrical discharge. Charging/beam audio cuts off when the weapon ends or the tank dies.

Idle and loaded engine layers blend with throttle and actual track velocity; track clatter follows movement, including turns. Engine/track/weapon loops fade out during pause, editing and death. Simulation events provide hull collisions, projectile impacts, fracture/debris, explosions, bridge collapse and sinking. Barrel buckling has a hollow shell-crumpling sound; yielding metal fences use the same low, hollow crumpling character with a shorter, more damped tail. These trigger on accepted plastic work, not mere contact or an already bent object. They coalesce per frame with a 280 ms cooldown. Pickup and reload-ready now use short, subdued mechanical cues, mixed at 35% / 22% of the former cue gain before the interface-volume control. Respawn, countdown and result cues complete the feedback.

Effects attenuate to silence over 45 metres from the player's tank. Nearby unseen impacts can still be heard; music requires visual contact. Multiple broken cells coalesce into one crash per frame, with per-sound cooldowns and a six-transient-per-frame budget. Browser playback has a 32-voice limit and a gentle compressor. Audio never changes simulation timing, forces or outcomes.

Destroying decorative shrubs, crates and timber adds a quiet leafy rustle, dry wood crunch or deeper wooden snap respectively. Tank contact, projectile/beam sweeps and explosions all use the same one-time destruction event. These cues use the effects volume and distance attenuation, coalesce by material, and have a 160 ms cooldown; the existing visual burst does not retrigger them as it fades. Decorations remain nonphysical.

Desktop mixing additionally limits transients to 32 overall and three per effect, smoothly ramps starts/volume changes/stops, and soft-limits exceptional summed peaks. Loop tails and heads are joined in order without gaps or overlaps, including loops shorter than the device buffer. Tracks, AP reports, impacts, fracture, fences and reloads are low-pass filtered during asset generation to remove abrasive high-frequency noise; track clatter also has a quiet, smooth loop seam. These filters add no runtime processing cost.

Movement blends subtle surface textures with the existing engine and tread clatter: tighter clatter on road, softer scrape on sand, dry rustle in tall grass, hollow rumble on bridges, and a light gritty layer on elevation. The terrain under the tank selects the mix, including intact bridge crossings. Transitions settle over roughly 350 ms without restarting loops. Actual track speed controls level, including turns; stopping, pause, death and mute silence the surface layers with the other vehicle effects.

## Controls

Choose **Sound Settings** from the title/lobby/pause/results screens, or press **F7** anywhere. The dedicated screen pauses gameplay and blocks firing/editor shortcuts until closed. **Done / Esc** returns to the previous screen and preserves the prior pause state.

Four saved sliders offer 0–100% **Master**, **Music**, **Battle & Vehicle Effects**, and **Pickup, Reload & UI Cues**. The last two categories are independent. Drag a slider or use its +/- buttons (5% increments). Mute, **Test Effect**, and **Restore Defaults** are available in the same panel.

- **F8:** toggle all sound.
- **F9 / F10:** reduce music/effects by 10 percentage points, wrapping from off to 100%.
- Defaults: master 100%, music 65%, effects 100%, interface cues 60%.

Preferences save in `audio-settings.txt` on desktop and `fracture.audio.v1` in browser storage. The versioned packed format migrates the previous mute/music/effects settings and preserves intentional silence. Preferences are shared between applications and separate from maps.

Browsers unlock audio on the first click/tap/key press. Hidden tabs suspend audio, discard unfinished transient sounds, and resume loops when visible again. A browser's own site-mute setting still applies.

## Assets and implementation

Effects: `python3 tools/generate-audio.py`, requiring NumPy and ffmpeg. `--only barrel-crush fence-crunch pickup reload` regenerates selected effects without touching the music.

Music: `python3 tools/render-score.py --soundfont /path/to/GeneralUser-GS.sf2`, requiring NumPy, libfluidsynth and ffmpeg. This contains the original written score, orchestration, seeded performance variation and offline rendering. Only the rendered Ogg tracks ship; normal builds need none of these authoring tools. The manifest records lengths, tempos and levels.

Instrument source: **GeneralUser GS v2.0.3**, S. Christian Collins. [Project and download information](https://github.com/mrbumpy409/GeneralUser-GS). The author permits private and commercial music creation; the source license is preserved as `assets/audio/GeneralUser-GS-LICENSE.txt`. The exact rendering bank SHA-256 is `9575028c7a1f589f5770fccc8cff2734566af40cd26ed836944e9a5152688cfe`. Music compositions are original to this game; no existing WWII melody or recording is used.

`crates/sandbox/src/audio.rs` owns mixing, event selection and music state. `audio/settings.rs` owns preferences; `audio/panel.rs` owns the modal settings UI. `Lab` collects bounded, coalesced cues from real ticks; typed `Fired` events carry ammunition and `Collided` events report significant hull contact. The simulation has no audio dependency.

Macroquad's audio feature supplies native playback through a patched `quad-snd` 0.2.8 in `vendor/quad-snd`, selected by the root Cargo patch. See its `PATCHES.md` for provenance and fixes. Linux requires the ALSA runtime (`libasound.so.2`); the build supplies a linker script so the development symlink is unnecessary. The two launcher build scripts explicitly allow quad-snd's known WebAssembly imports. `web/audio.js` implements those imports with smoothed Web Audio gains, gesture unlock, tab suspension and voice cleanup.

## Verification

`cargo test -p quad-snd --lib --locked` runs the native mixer regression tests without requiring an audio device: continuous loop wraps, short loops, exact sample endings, start/stop/volume ramps, transient budgets and peak limiting. Run this explicitly in addition to workspace tests because the vendored dependency is excluded from workspace membership.

`node tools/audio-smoke.mjs` uses the usual isolated Chrome setup. It checks asset decoding, an actual nonzero output signal, gesture unlock, visible/occluded enemy transitions, engine response, weapon cues, pause, live-match input isolation, settings sliders, mute output and persisted settings. Unit tests cover equal-power fades, contact hold, pause state, effect selection, attenuation and settings migration. Physics tests check that actual barrel crushing and fence yielding emit their respective events. Existing game and ammunition browser suites cover session/weapon behavior with audio enabled.
