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
  assert.equal((await stats()).broken, 0);
  await screenshot('sandbox-ready');

  await key('KeyP', 'p', 80);
  const paused = await stats();
  await delay(200);
  assert.equal((await stats()).ticks, paused.ticks, 'pause freezes simulation');
  await key('KeyN', 'n', 78);
  assert.equal((await stats()).ticks, paused.ticks + 1, 'single-step advances one tick');
  await key('Space', ' ', 32);
  assert.equal((await stats()).shots, 0, 'paused shot is queued');
  await key('KeyN', 'n', 78);
  assert.equal((await stats()).shots, 1, 'step executes queued shot');
  assert.equal((await stats()).shells, 1, 'shell exists during flight');
  await screenshot('sandbox-shell');
  for (let i = 0; i < 8 && (await stats()).explosions === 0; i++) await key('KeyN', 'n', 78);
  assert.equal((await stats()).explosions, 1, 'swept shell hits during single stepping');
  await screenshot('sandbox-impact');
  await key('KeyP', 'p', 80);
  await until('wasm_exports.lab_broken() > 0', 'masonry fractures');
  const fractureTick = (await stats()).ticks;
  await until(`wasm_exports.lab_ticks() > ${fractureTick + 60}`, 'debris moves');
  await key('KeyP', 'p', 80);
  await screenshot('sandbox-fracture');
  await key('Tab', 'Tab', 9);
  await screenshot('sandbox-loads');
  assert.equal(await evaluate('document.activeElement.id'), 'glcanvas', 'Tab keeps canvas focus');
  const impacted = await stats();
  assert.equal(impacted.shells, 0, 'shell is consumed on impact');
  assert.equal(impacted.explosions, 1, 'one explosion per hit');
  assert.ok(Math.abs(impacted.x + 7) < 0.04, 'tracks arrest recoil');
  assert.equal(impacted.unconverged, 0, 'masonry stress converges');
  assert.ok(impacted.refined > 0, 'impact adds local detail inside the barrier');
  console.log('Masonry after shot:', impacted);
  await key('Tab', 'Tab', 9);
  await key('KeyP', 'p', 80);
  await until('wasm_exports.lab_cleared() > 0', 'tiny fragments disappear');
  await until(`wasm_exports.lab_ticks() > ${impacted.ticks + 550}`, 'debris cleanup settles');
  await key('KeyP', 'p', 80);
  await screenshot('sandbox-cleared');

  await key('KeyR', 'r', 82);
  assert.equal((await stats()).broken, 0);
  assert.equal((await stats()).shots, 0);
  await key('Digit2', '2', 50);
  assert.equal((await stats()).cells, 148, 'cantilever scene selected');
  await key('KeyP', 'p', 80);
  await click(845, 478);
  await until('wasm_exports.lab_shots() === 1 && wasm_exports.lab_broken() > 0', 'mouse shot fractures cantilever');
  console.log('Cantilever after shot:', await stats());

  await key('Digit3', '3', 51);
  assert.equal((await stats()).chunks, 15, 'loose bodies scene selected');
  await click(754, 442);
  await until('wasm_exports.lab_explosions() === 1', 'rock impact');
  const rockTick = (await stats()).ticks;
  await until(`wasm_exports.lab_ticks() > ${rockTick + 850}`, 'rock chunks settle');
  const rocks = await stats();
  assert.ok(rocks.broken >= 2 && rocks.broken <= 20, 'localized rock splitting');
  assert.ok(rocks.chunks > 15 && rocks.chunks < 30 && rocks.clearedArea <= 0.225, 'substantial rock chunks remain');
  assert.equal(rocks.explosions, 1, 'no secondary explosions');
  await screenshot('sandbox-rock-chunks');
  assert.equal(rocks.unconverged, 0, 'rock stress converges');
  console.log('Rock field after one shell:', rocks);
  await click(404, 40); // First scene in the five-item header.
  assert.equal((await stats()).cells, 252, 'scene button selected masonry');
  assert.equal((await stats()).shots, 0, 'UI click did not fire into world');

  const initialX = (await stats()).x;
  await cdp('Input.dispatchKeyEvent', { type: 'keyDown', code: 'KeyW', key: 'w', windowsVirtualKeyCode: 87 });
  await delay(950);
  await cdp('Input.dispatchKeyEvent', { type: 'keyUp', code: 'KeyW', key: 'w', windowsVirtualKeyCode: 87 });
  assert.ok((await stats()).x > initialX + 0.5, 'W drives tracks forward');
  await delay(800);
  const stopped = await stats();
  await delay(300);
  assert.ok(Math.abs((await stats()).x - stopped.x) < 0.02, 'released tracks brake');
  await cdp('Input.dispatchKeyEvent', { type: 'keyDown', code: 'KeyD', key: 'd', windowsVirtualKeyCode: 68 });
  await delay(500);
  await cdp('Input.dispatchKeyEvent', { type: 'keyUp', code: 'KeyD', key: 'd', windowsVirtualKeyCode: 68 });
  assert.ok((await stats()).angle > stopped.angle + 0.1, 'D steers the tank');
  const turnReleased = (await stats()).angle;
  await delay(350);
  assert.ok(Math.abs((await stats()).angle - turnReleased) < 0.08, 'rotation stops promptly after releasing steering');
  await key('KeyH', 'h', 72);
  await screenshot('sandbox-manual');
  await key('KeyH', 'h', 72);

  await key('KeyP', 'p', 80);
  const zoomBefore = await evaluate('wasm_exports.lab_zoom()');
  await evaluate(`window.zoomFrames = []; window.captureZoom = true;
    function sampleZoom() {
      if (!window.captureZoom) return;
      window.zoomFrames.push({z: wasm_exports.lab_zoom(), x: wasm_exports.lab_camera_x(), y: wasm_exports.lab_camera_y()});
      requestAnimationFrame(sampleZoom);
    } requestAnimationFrame(sampleZoom);`);
  await cdp('Input.dispatchMouseEvent', {type: 'mouseMoved', x: 925, y: 380});
  await cdp('Input.dispatchMouseEvent', {type: 'mouseWheel', x: 925, y: 380, deltaX: 0, deltaY: -100});
  await delay(650);
  await evaluate('window.captureZoom = false');
  const frames = await evaluate('window.zoomFrames');
  const targetZoom = zoomBefore * Math.exp(0.11);
  const changed = frames.filter(f => f.z > zoomBefore + 0.0001);
  assert.ok(changed.length >= 4, 'zoom moves across multiple frames');
  assert.ok(changed[0].z < targetZoom - 0.001, 'wheel does not snap to target zoom');
  assert.ok(Math.abs(changed.at(-1).z - targetZoom) < 0.002, 'pixel wheel input has modest sensitivity');
  const baseScale = (723 - 95) / 15;
  const anchors = changed.map(f => ({x: f.x + (925-720)/(baseScale*f.z), y: f.y + (380-441.5)/(baseScale*f.z)}));
  assert.ok(anchors.every(p => Math.hypot(p.x-anchors[0].x, p.y-anchors[0].y) < 0.0001), 'zoom preserves point under cursor');
  await screenshot('sandbox-smooth-zoom');
  await key('Home', 'Home', 36);

  await cdp('Emulation.setDeviceMetricsOverride', { width: 900, height: 700, deviceScaleFactor: 2, mobile: false });
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5 && document.getElementById("status").hidden', 'high-DPI startup');
  await delay(350);
  await click(344, 26); // Cantilever at 0.625 UI scale.
  assert.equal((await stats()).cells, 148, 'scene selection works at high DPI');
  await screenshot('sandbox-small-hidpi');
  assert.deepEqual(errors, [], 'no browser runtime exceptions');
  console.log('PASS: startup, rendering, pause, step, shell impact/removal, recoil, fracture/cleanup, track driving/rotation braking, rock chunk retention, smooth cursor-anchored zoom, manual, overlay, scene controls, resize.');
} catch (error) {
  console.error('Browser exceptions:', JSON.stringify(errors));
  console.error('Loading status:', await evaluate('document.getElementById("status").textContent').catch(() => 'unavailable'));
  await screenshot('sandbox-error').catch(() => {});
  throw error;
} finally {
  ws.close();
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(() => {});
}
