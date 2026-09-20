// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const url = process.env.SANDBOX_URL || 'http://127.0.0.1:8000';
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
  for (let i = 0; i < 1000; i++) {
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

let original;
const map=()=>evaluate('JSON.parse(localStorage.getItem("fracture.level.v1"))');
try {
  await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5 && document.getElementById("status").hidden','startup');
  original=await evaluate('localStorage.getItem("fracture.level.v1")');
  const draft={version:1,name:'Existing map',size:[48,30],items:[{kind:'spawn',team:'blue',position:[-18,0],angle:0}]};
  await evaluate(`localStorage.setItem('fracture.level.v1',${JSON.stringify(JSON.stringify(draft))})`);await cdp('Page.reload');await until('wasm_exports?.lab_ticks?.()>5','reload');
  await key('KeyM','m',77);await until('wasm_exports.level_editor_active()===1','editor');
  await click(490,40);await click(560,326);await until('wasm_exports.level_editor_items()===31','demo menu');
  const demo=await map();assert.equal(demo.name,'Quarry Crossing');assert.equal(demo.items.length,31);
  const download=await fetch(`${url}/levels/quarry-crossing.json`).then(r=>r.json());assert.deepEqual(download,demo,'download and built-in map agree');
  await screenshot('demo-quarry-editor');
  await click(580,40);assert.deepEqual(await map(),draft,'loading a demo can be undone');
  await click(680,40);assert.deepEqual(await map(),demo,'redo restores demo');
  await click(1120,40);await until('wasm_exports.level_editor_active()===0 && wasm_exports.lab_flags()===2 && wasm_exports.lab_bridges()===1','play demo');
  assert.equal((await stats()).broken,0);assert.ok((await stats()).cells<1600);
  await screenshot('demo-quarry-play');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.lab_tank_x()>-1.0','crossing the bridge');await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await delay(250);await screenshot('demo-quarry-crossing');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});await until('wasm_exports.lab_winner()===1','blue victory');
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});assert.equal(await evaluate('wasm_exports.lab_tank_alive()'),1);
  await screenshot('demo-quarry-victory');
  await key('KeyM','m',77);assert.deepEqual(await map(),demo,'playtest preserves authored demo');await click(950,40);await click(1120,40);
  await until('wasm_exports.lab_tank_x()>18.9','red spawn');await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.lab_winner()===2','red victory');await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await key('KeyM','m',77);await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});await delay(300);await screenshot('demo-quarry-high-dpi');
  assert.deepEqual(errors,[]);console.log('PASS: demo menu, canonical JSON download, undo/redo, stable start, both team bridge runs and victories, preserved map and high DPI.');
} catch(error) {await screenshot('demo-quarry-failure');throw error;} finally {
  if(original!==undefined) await evaluate(`(()=>{const value=${JSON.stringify(original)};if(value===null)localStorage.removeItem('fracture.level.v1');else localStorage.setItem('fracture.level.v1',value);})()`).catch(()=>{});
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});ws.close();
}
