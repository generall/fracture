// Standard browser Gamepad mapping; Steam Input's Gamepad template works too.
(() => {
  let selected = null;
  miniquad_add_plugin({ name: 'controller', version: 1, register_plugin(imports) {
    imports.env.controller_handheld_mode = () =>
      new URLSearchParams(location.search).get('deck') === '1' ? 1 : 0;
    imports.env.controller_poll = ptr => {
      const axes = new Float32Array(wasm_memory.buffer, ptr, 4);
      axes.fill(0);
      if (document.hidden) { selected = null; return 0; }
      const pads = navigator.getGamepads?.() || [];
      let pad = selected === null ? null : pads[selected];
      if (!pad?.connected) {
        if (selected !== null) { selected = null; return 0; }
        pad = Array.from(pads).find(p => p?.connected && p.mapping === 'standard');
        if (!pad) return 0;
        selected = pad.index;
      }
      for (let i = 0; i < 4; i++) axes[i] = Number.isFinite(pad.axes[i]) ? pad.axes[i] : 0;
      let state = 0x80000000;
      for (const [button, bit] of [[0,0],[1,1],[2,2],[3,3],[5,4],[9,5],[7,6],[6,7],[12,8],[13,9],[14,10],[15,11]]) {
        if ((pad.buttons[button]?.value || 0) > 0.25) state |= 1 << bit;
      }
      return state >>> 0;
    };
  }});
})();
