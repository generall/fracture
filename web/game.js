// Local match persistence, shared by both application entry points.
(() => {
  const key = 'fracture.match-map.v1';
  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  miniquad_add_plugin({
    name: 'game', version: 1,
    register_plugin(imports) {
      imports.env.game_map_restore = (ptr, capacity) => {
        try {
          const bytes = encoder.encode(localStorage.getItem(key) || '');
          if (bytes.length > capacity) return -1;
          new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
          return bytes.length;
        } catch { return -1; }
      };
      imports.env.game_map_store = (ptr, len) => {
        try { localStorage.setItem(key, decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len))); return 0; }
        catch { return -1; }
      };
    }
  });
})();
