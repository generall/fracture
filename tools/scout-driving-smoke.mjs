// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --mute-audio --remote-debugging-port=9227.
// Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.GAME_URL || 'http://127.0.0.1:8000');
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
async function until(expression, label, attempts = 160) {
  for (let i = 0; i < attempts; i++) {
    if (await evaluate(expression)) return;
    await delay(50);
  }
  throw new Error(`Timed out: ${label}`);
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
async function importMap(level){const json=JSON.stringify(level);await evaluate(`(()=>{const bytes=new TextEncoder().encode(${JSON.stringify(json)});const ptr=wasm_exports.level_import_buffer(bytes.length);new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);wasm_exports.level_import_commit();})()`);await delay(200);}

const hold=(type,code,key,n)=>cdp('Input.dispatchKeyEvent',{type,code,key,windowsVirtualKeyCode:n});
async function key(code,k,n){await hold('keyDown',code,k,n);await delay(60);await hold('keyUp',code,k,n);await delay(100);}
const fixture={version:1,name:'Scout steering test',size:[160,160],enemy_activity:'static',items:[
 {kind:'spawn',team:'blue',position:[-50,0],angle:0},
 {kind:'spawn',team:'red',position:[50,0],angle:Math.PI},
 {kind:'flag',team:'blue',position:[-65,0]}, {kind:'flag',team:'red',position:[65,0]}
]};
const state=()=>evaluate('({x:wasm_exports.lab_tank_x(),y:wasm_exports.lab_tank_y(),angle:wasm_exports.lab_tank_angle(),ticks:wasm_exports.lab_ticks()})');
async function speed(){const a=await state();await delay(300);const b=await state();return Math.hypot(b.x-a.x,b.y-a.y)*120/(b.ticks-a.ticks);}
try {
 await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
 await cdp('Network.enable');await cdp('Network.setCacheDisabled',{cacheDisabled:true});
 await cdp('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 await cdp('Page.navigate',{url});
 await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden','startup');
 await click(260,375);await until('wasm_exports.game_screen()===1','lobby');
 await importMap(fixture);await click(1100,705);await until('wasm_exports.game_phase()===2','deployment');
 await click(285,540);await until('wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===0','scout');
 const parked=await state();await hold('keyDown','KeyD','d',68);await delay(1000);await hold('keyUp','KeyD','d',68);
 const steered=await state();assert.ok(Math.abs(steered.angle-parked.angle)<0.001,'no pivot at rest');
 assert.ok(Math.hypot(steered.x-parked.x,steered.y-parked.y)<0.001,'steering alone does not drive');
 await hold('keyDown','KeyW','w',87);await delay(3400);
 const straight=await speed();assert.ok(straight>9.7 && straight<10.2,`full throttle ${straight}`);
 await hold('keyDown','KeyD','d',68);await delay(1400);
 const turning=await speed();assert.ok(turning>=straight*0.96,`turn speed ${turning} versus straight ${straight}`);
 assert.ok(Math.abs((await state()).angle)>0.8,'moving car follows a curve');
 await screenshot('scout-car-turn');
 await hold('keyUp','KeyW','w',87);await hold('keyUp','KeyD','d',68);await delay(1800);
 assert.ok(await speed()<0.05,'release brakes stop car');
 assert.equal(errors.length,0,JSON.stringify(errors));
 console.log(`PASS: scout cannot pivot, turns at full speed (${straight.toFixed(2)} straight / ${turning.toFixed(2)} turning m/s), and brakes on release`);
} catch(e) {await screenshot('scout-driving-error');throw e;}
finally {ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});}
