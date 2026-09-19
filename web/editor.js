// Loaded after miniquad and before the WASM module. Objects stay on this device.
(() => {
  const storageKey = 'fracture.object.v1';
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const encoder = new TextEncoder();
  const read = (ptr, len) => decoder.decode(new Uint8Array(wasm_memory.buffer, ptr, len));
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json,application/json';
  input.hidden = true;
  document.body.append(input);
  const deliver = text => {
    const bytes = encoder.encode(text);
    const ptr = wasm_exports.editor_import_buffer(bytes.length);
    if (!ptr) return;
    new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
    wasm_exports.editor_import_commit();
    surface.focus();
  };
  input.addEventListener('change', async () => {
    const file = input.files[0];
    if (!file) return;
    try {
      if (file.size > 65536) wasm_exports.editor_import_fail(1);
      else deliver(await file.text());
    } catch { wasm_exports.editor_import_fail(2); }
    input.value = '';
    surface.focus();
  });
  miniquad_add_plugin({
    name: 'object_editor', version: 1,
    register_plugin(imports) {
      imports.env.object_library_store = (ptr, len) => {
        try { localStorage.setItem('fracture.objects.v1', read(ptr, len)); return 0; }
        catch { return -1; }
      };
      imports.env.object_library_restore = (ptr, capacity) => {
        try {
          const bytes = encoder.encode(localStorage.getItem('fracture.objects.v1') || '');
          if (bytes.length > capacity) return -1;
          new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
          return bytes.length;
        } catch { return -1; }
      };
      imports.env.object_store = (ptr, len) => {
        try { localStorage.setItem(storageKey, read(ptr, len)); return 0; }
        catch { return -1; }
      };
      imports.env.object_restore = (ptr, capacity) => {
        try {
          const bytes = encoder.encode(localStorage.getItem(storageKey) || '');
          if (bytes.length > capacity) return -1;
          new Uint8Array(wasm_memory.buffer, ptr, bytes.length).set(bytes);
          return bytes.length;
        } catch { return -1; }
      };
      imports.env.object_download = (ptr, len) => {
        try {
          const json = read(ptr, len);
          const name = JSON.parse(json).name.replace(/[^a-z0-9_-]/gi, '-').slice(0, 40) || 'object';
          const url = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
          const link = document.createElement('a');
          link.href = url; link.download = `${name}.json`;
          document.body.append(link); link.click(); link.remove();
          setTimeout(() => URL.revokeObjectURL(url), 1000);
          return 0;
        } catch { return -1; }
      };
      imports.env.object_pick_file = () => input.click();
    }
  });
})();
