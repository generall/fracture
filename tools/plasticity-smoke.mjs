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

// Exercise actual canvas input, browser file upload/download, and persistence.
const draft = () => evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
async function typeText(value) {
  for (const ch of value) await cdp('Input.dispatchKeyEvent', { type: 'char', text: ch, key: ch });
  await delay(70);
}
async function field(x,y,value) {
  await click(x,y);
  await typeText(value);
  await key('Enter','Enter',13);
}
async function upload(path) {
  const {root}=await cdp('DOM.getDocument');
  const {nodeId}=await cdp('DOM.querySelector',{nodeId:root.nodeId,selector:'input[type=file]'});
  await cdp('DOM.setFileInputFiles',{nodeId,files:[path]});
  await delay(250);
}
async function tab(index) {await click(75+116*index,154);}
async function preset(index) {await click(435,40);await click(500,132+40*index);}
async function fileAction(y) {await click(435,40);await click(500,y);}
// Exercise deformable authoring and the real WASM tank/fence contact path.
const keys=['fracture.object.v1','fracture.objects.v1'];
let originals;
const plastic=()=>evaluate('({work:wasm_exports.lab_plastic_work(),strain:wasm_exports.lab_plastic_strain(),broken:wasm_exports.lab_broken(),unconverged:wasm_exports.lab_unconverged()})');
async function hold(code,key,keyCode,ms) {
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code,key,windowsVirtualKeyCode:keyCode});
  await delay(ms);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code,key,windowsVirtualKeyCode:keyCode});
  await delay(60);
}
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5','startup');
  originals=await evaluate(`JSON.stringify(${JSON.stringify(keys)}.map(k=>localStorage.getItem(k)))`);
  await key('KeyO','o',79);await key('KeyL','l',76);
  await click(300,198);await click(1100,198);await screenshot('fence-library');
  await click(450,302);
  let a=await draft();assert.equal(a.name,'metal fence');assert.ok(a.components.plasticity);
  assert.deepEqual(a.color,[110,184,255]);
  await tab(2);await click(180,454); // Direct Yield & Hardening page.
  await screenshot('plasticity-editor');
  await field(270,340,'25');await field(270,392,'1.5');
  await field(270,444,'3');await field(270,496,'0.8');
  const settings={yield_stress:25000,flow_rate:1.5,hardening:3,max_strain:0.8};
  assert.deepEqual((await draft()).components.plasticity,settings);
  // Switching behavior is undoable, and the previous material parameters return.
  await click(180,272);assert.ok(!(await draft()).components.plasticity);
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'ControlLeft',key:'Control',windowsVirtualKeyCode:17,modifiers:2});
  await key('KeyZ','z',90);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'ControlLeft',key:'Control',windowsVirtualKeyCode:17});
  assert.deepEqual((await draft()).components.plasticity,settings);
  await key('KeyL','l',76);await click(300,710);await key('Escape','Escape',27);
  assert.deepEqual(await evaluate("JSON.parse(localStorage.getItem('fracture.objects.v1')).objects.find(a=>a.name==='metal fence').components.plasticity"),settings);
  await cdp('Page.reload');await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5','reload');
  await key('KeyO','o',79);assert.deepEqual((await draft()).components.plasticity,settings);
  await click(1160,40);await hold('KeyW','w',87,4200);
  const loaded=await plastic();assert.ok(loaded.work>0 && loaded.strain>0,JSON.stringify(loaded));
  assert.equal(loaded.broken,0);assert.equal(loaded.unconverged,0);
  await screenshot('fence-bent');
  await hold('KeyS','s',83,1200);await delay(200);const released=await plastic();
  await delay(600);assert.deepEqual(await plastic(),released);await screenshot('fence-released');
  await key('KeyR','r',82);assert.equal((await plastic()).work,0);
  assert.deepEqual(errors,[]);
  console.log('PASS: metal fence starter, deformable/destructible toggle, yield/flow/hardening/limit, undo, save/reload, real tank bends, permanent rest shape, reset.');
} finally {
  if(originals) {
    const values=JSON.parse(originals);
    await evaluate(`${JSON.stringify(keys)}.forEach((k,i)=>{const v=${JSON.stringify(values)}[i];if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)})`).catch(()=>{});
  }
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
