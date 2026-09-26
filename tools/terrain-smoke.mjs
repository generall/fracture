// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.SANDBOX_URL || 'http://127.0.0.1:8000/sandbox.html');
sandboxUrl.searchParams.delete('mode');
const url = sandboxUrl.href;
const page = await fetch(`${debugUrl}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' }).then(r => r.json());
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let seq = 0;
const pending = new Map();
const errors = [];
ws.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails);
  if (message.id) {
    const request = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) request.reject(new Error(JSON.stringify(message.error)));
    else request.resolve(message.result);
  }
};
function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const response = await cdp('Runtime.evaluate', { expression, returnByValue: true });
  if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
  return response.result.value;
}
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(expression, label) {
  for (let i = 0; i < 160; i++) {
    if (await evaluate(expression)) return;
    await delay(50);
  }
  throw new Error(`Timed out: ${label}`);
}
async function key(code, key, keyCode) {
  await cdp('Input.dispatchKeyEvent', { type: 'keyDown', code, key, windowsVirtualKeyCode: keyCode });
  await delay(35);
  await cdp('Input.dispatchKeyEvent', { type: 'keyUp', code, key, windowsVirtualKeyCode: keyCode });
  await delay(50);
}
async function click(x, y) {
  await cdp('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
  await delay(30);
  await cdp('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
  await delay(30);
  await cdp('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
  await delay(60);
}
async function screenshot(name) {
  const { data } = await cdp('Page.captureScreenshot', { format: 'png' });
  await writeFile(`target/screenshots/${name}.png`, Buffer.from(data, 'base64'));
}
const stats = () => evaluate('({ticks: wasm_exports.lab_ticks(), broken: wasm_exports.lab_broken(), shots: wasm_exports.lab_shots(), bodies: wasm_exports.lab_bodies(), shells: wasm_exports.lab_shells(), explosions: wasm_exports.lab_explosions(), cleared: wasm_exports.lab_cleared(), clearedArea: wasm_exports.lab_cleared_area(), cells: wasm_exports.lab_cells(), chunks: wasm_exports.lab_chunks(), refined: wasm_exports.lab_refinements(), unconverged: wasm_exports.lab_unconverged(), x: wasm_exports.lab_tank_x(), y: wasm_exports.lab_tank_y(), angle: wasm_exports.lab_tank_angle()})');

const terrain = () => evaluate('({alive: wasm_exports.lab_tank_alive(), bridges: wasm_exports.lab_bridges(), terrain: wasm_exports.lab_terrain()})');
const hold = (type, code, key, windowsVirtualKeyCode) => cdp('Input.dispatchKeyEvent', { type, code, key, windowsVirtualKeyCode });
try {
  await mkdir('target/screenshots', { recursive: true });
  await cdp('Runtime.enable');
  await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5 && document.getElementById("status").hidden', 'WASM startup');
  await key('Digit9', '9', 57);
  await until('wasm_exports.lab_bridges() === 1', 'terrain scene');
  assert.deepEqual(await terrain(), {alive: 1, bridges: 1, terrain: 1});
  await screenshot('terrain-ready');
  for (const [code, number] of [['Digit5', 5], ['Digit6', 6], ['Digit7', 7], ['Digit8', 8]]) {
    await key(code, String(number), 48 + number);
    const shots = (await stats()).shots;
    await click(720, 441.5);
    await until(`wasm_exports.lab_shots() > ${shots}`, 'weapon clears the deck');
    await delay(600);
    assert.equal((await terrain()).bridges, 1, 'deck is not a weapon target');
    assert.equal((await stats()).broken, 0);
    assert.equal((await stats()).refined, 0);
    assert.equal(await evaluate('wasm_exports.lab_ablated()'), 0);
  }
  await key('Digit5', '5', 53);
  await key('KeyR', 'r', 82);
  await hold('keyDown', 'KeyW', 'w', 87);
  await until('wasm_exports.lab_tank_x() > 5', 'cross intact bridge');
  await hold('keyUp', 'KeyW', 'w', 87);
  assert.equal((await terrain()).alive, 1);
  assert.equal((await stats()).broken, 0);
  await key('KeyR', 'r', 82);
  await hold('keyDown', 'KeyW', 'w', 87);
  await until('wasm_exports.lab_tank_x() > -0.8', 'tank on span');
  await hold('keyUp', 'KeyW', 'w', 87);
  await delay(450);
  assert.equal((await terrain()).terrain, 4);
  await screenshot('terrain-crossing');
  const scale = (900 - 177 - 95) / 15;
  for (let n = 0; n < 12 && (await terrain()).bridges; n++) {
    await click(720 - 3 * scale, 441.5 + 2.05 * scale);
    await delay(470);
  }
  assert.equal((await terrain()).bridges, 0, 'shell destroys a bearing from the deck');
  assert.equal((await terrain()).alive, 0, 'occupant is lost');
  await screenshot('terrain-collapse');
  await delay(1600);
  await screenshot('terrain-sunk');
  const lost = await stats();
  await hold('keyDown', 'KeyW', 'w', 87);
  await key('KeyF', 'f', 70);
  await delay(350);
  await hold('keyUp', 'KeyW', 'w', 87);
  assert.equal((await stats()).x, lost.x, 'destroyed tank cannot drive');
  assert.equal((await stats()).shots, lost.shots, 'destroyed tank cannot fire');
  await key('KeyR', 'r', 82);
  assert.equal((await terrain()).alive, 1, 'rebuild restores tank');
  assert.equal((await terrain()).bridges, 1, 'rebuild restores bridge');
  assert.equal(errors.length, 0, JSON.stringify(errors));
  console.log(JSON.stringify({result: 'PASS', checks: ['terrain scene', 'all weapons clear deck', 'bridge crossing', 'booth shooting', 'occupant sinking', 'disabled controls', 'rebuild'], lost}, null, 2));
} finally {
  await hold('keyUp', 'KeyW', 'w', 87).catch(() => {});
  ws.close();
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(() => {});
}
