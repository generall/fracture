// Macroquad audio bridge: smooth gains, bounded voices, browser unlock and tab suspension.
// Original implementation; uses the same imports as quad-snd 0.2.8.
(() => {
  let context, bus, serial = 1, unlocked = false;
  const buffers = new Map(), voices = new Map();
  const key = 'fracture.audio.v1';
  const debug = { loaded: 0, played: 0, active: 0, errors: 0, context: () => context };
  window.fractureAudio = debug;
  function init() {
    if (context) return;
    const Context = window.AudioContext || window.webkitAudioContext;
    context = new Context();
    bus = context.createDynamicsCompressor();
    bus.threshold.value = -8; bus.knee.value = 10; bus.ratio.value = 4;
    bus.attack.value = .003; bus.release.value = .2;
    bus.connect(context.destination);
    const analyser = context.createAnalyser(); analyser.fftSize = 2048;
    bus.connect(analyser);
    debug.rms = () => {
      const samples = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(samples);
      return Math.sqrt(samples.reduce((sum, x) => sum + x*x, 0) / samples.length);
    };
    debug.voices = () => [...voices.values()].map(v => ({sound:v.sound,loop:v.loop,gain:v.gain.gain.value}));
  }
  function stop(id) {
    const voice = voices.get(id);
    if (!voice) return;
    voices.delete(id); voice.source.onended = null;
    try { voice.source.stop(); } catch { /* already ended */ }
    voice.source.disconnect(); voice.gain.disconnect();
    debug.active = voices.size;
  }
  function volume(id, value) {
    const voice = voices.get(id);
    if (voice) voice.gain.gain.setTargetAtTime(Math.max(0, Math.min(1, value)), context.currentTime, .018);
  }
  function unlock() {
    unlocked = true;
    if (context && !document.hidden) context.resume().catch(() => {});
  }
  document.addEventListener('pointerdown', unlock);
  document.addEventListener('keydown', unlock);
  document.addEventListener('visibilitychange', () => {
    if (!context) return;
    if (document.hidden) {
      for (const [id, voice] of voices) if (!voice.loop) stop(id);
      context.suspend().catch(() => {});
    } else if (unlocked) context.resume().catch(() => {});
  });
  window.addEventListener('pagehide', () => { for (const id of voices.keys()) stop(id); });
  miniquad_add_plugin({ name: 'fracture_audio', version: 1, register_plugin(imports) {
    Object.assign(imports.env, {
      audio_init: init,
      audio_add_buffer(ptr, len) {
        const id = serial++;
        context.decodeAudioData(wasm_memory.buffer.slice(ptr, ptr + len)).then(buffer => {
          buffers.set(id, buffer); debug.loaded++;
        }).catch(error => {
          debug.errors++;
          console.warn('Audio asset unavailable', error);
          buffers.set(id, context.createBuffer(2, 1, 44100));
        });
        return id;
      },
      audio_source_is_loaded: id => Number(buffers.has(id)),
      audio_play_buffer(sound, gain, loop) {
        if (!buffers.has(sound)) return 0;
        if (voices.size >= 32) {
          const oldest = [...voices].find(([,v]) => !v.loop);
          if (oldest) stop(oldest[0]); else return 0;
        }
        const id = serial++, source = context.createBufferSource(), node = context.createGain();
        source.buffer = buffers.get(sound); source.loop = Boolean(loop);
        node.gain.value = Math.max(0, Math.min(1, gain));
        source.connect(node); node.connect(bus);
        voices.set(id, { source, gain: node, sound, loop: Boolean(loop) });
        source.onended = () => stop(id);
        source.start(); debug.played++; debug.active = voices.size;
        return id;
      },
      audio_source_set_volume(sound, gain) { for (const [id,v] of voices) if (v.sound === sound) volume(id,gain); },
      audio_source_stop(sound) { for (const [id,v] of voices) if (v.sound === sound) stop(id); },
      audio_source_delete(sound) { for (const [id,v] of voices) if (v.sound === sound) stop(id); buffers.delete(sound); },
      audio_playback_stop: stop,
      audio_playback_set_volume: volume,
      audio_settings_load() { try { return Number(localStorage.getItem(key) || 0); } catch { return 0; } },
      audio_settings_store(value) { try { localStorage.setItem(key, String(value)); } catch { /* session settings still work */ } },
    });
  }});
})();
