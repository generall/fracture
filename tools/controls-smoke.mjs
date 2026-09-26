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

try {
  await mkdir('target/screenshots', { recursive: true });
  await cdp('Runtime.enable');
  await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5 && document.getElementById("status").hidden', 'WASM startup');
  const stopped = await stats();
  const angularDelta = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
  for (const [code, name, number, direction] of [
    ['KeyQ', 'q', 81, -1], ['KeyE', 'e', 69, 1],
    ['KeyJ', 'j', 74, -1], ['KeyK', 'k', 75, 1],
  ]) {
    const before = await evaluate('wasm_exports.lab_turret_angle()');
    await cdp('Input.dispatchKeyEvent', { type: 'keyDown', code, key: name, windowsVirtualKeyCode: number });
    await delay(500);
    await cdp('Input.dispatchKeyEvent', { type: 'keyUp', code, key: name, windowsVirtualKeyCode: number });
    await delay(100);
    const after = await evaluate('wasm_exports.lab_turret_angle()');
    assert.ok(angularDelta(after, before) * direction > 0.2, `${name} rotates the turret`);
    assert.ok(Math.abs((await stats()).angle - stopped.angle) < 0.08, `${name} does not steer the tracks`);
    await delay(350);
    assert.ok(Math.abs(angularDelta(await evaluate('wasm_exports.lab_turret_angle()'), after)) < 0.03,
      'idle cursor does not override keyboard aim after release');
  }
  const keyboardAngle = await evaluate('wasm_exports.lab_turret_angle()');
  await cdp('Input.dispatchMouseEvent', {type: 'mouseMoved', x: 720, y: 120});
  await delay(500);
  assert.ok(Math.abs(angularDelta(await evaluate('wasm_exports.lab_turret_angle()'), keyboardAngle)) > 0.2,
    'moving the mouse resumes mouse aiming');
  const shots = (await stats()).shots;
  await key('Space', ' ', 32);
  await until(`wasm_exports.lab_shots() === ${shots + 1}`, 'Space fires main gun');
  const ticks = (await stats()).ticks;
  await delay(150);
  assert.ok((await stats()).ticks > ticks, 'Space does not pause the sandbox');
  await key('KeyP', 'p', 80);
  const paused = (await stats()).ticks;
  await delay(150);
  assert.equal((await stats()).ticks, paused, 'P pauses the sandbox');
  await key('KeyH', 'h', 72);
  await screenshot('keyboard-controls');
  assert.deepEqual(errors, []);
  console.log('PASS: Q/E and J/K rotate turret without steering, release holds aim, mouse resumes aiming, Space fires and P pauses');
} catch (error) {
  console.error('Browser exceptions:', JSON.stringify(errors));
  console.error('Loading status:', await evaluate('document.getElementById("status").textContent').catch(() => 'unavailable'));
  await screenshot('sandbox-error').catch(() => {});
  throw error;
} finally {
  ws.close();
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(() => {});
}
