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
const fixture={version:1,name:'Mortar driving test',size:[160,160],enemy_activity:'static',items:[
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
 await click(995,540);await until('wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===2','mortar');
 await hold('keyDown','KeyW','w',87);await delay(450);
 const start=await speed();assert.ok(start>0.25 && start<1.2,`gradual launch ${start}`);
 await delay(850);
 const building=await speed();assert.ok(building>start+0.5 && building<2.4,`still accelerating ${building}`);
 await delay(4500);
 const cruise=await speed();assert.ok(cruise>3.45 && cruise<3.65,`steady cruising speed ${cruise}`);
 for(let i=0;i<2;i++) {
  const p=await evaluate('({x:wasm_exports.lab_tank_x(),y:wasm_exports.lab_tank_y(),cx:wasm_exports.lab_camera_x(),cy:wasm_exports.lab_camera_y(),z:wasm_exports.lab_zoom()})');
  const scale=Math.min((1280-90)/24,(800-95)/15)*p.z;
  const {data}=await cdp('Page.captureScreenshot',{format:'png',clip:{x:640+(p.x-p.cx)*scale-90,y:400+(p.y-p.cy)*scale-60,width:180,height:120,scale:3}});
  await writeFile(`target/screenshots/mortar-treads-${i}.png`,Buffer.from(data,'base64'));
  await delay(100);
 }
 await hold('keyUp','KeyW','w',87);await delay(2800);
 assert.ok(await speed()<0.05,'mortar stops after releasing drive');
 assert.equal(errors.length,0,JSON.stringify(errors));
 console.log(`PASS: mortar speed builds gradually (${start.toFixed(2)} → ${building.toFixed(2)} → ${cruise.toFixed(2)} m/s), then brakes; tread screenshots captured`);
} catch(e) {await screenshot('mortar-driving-error');throw e;}
finally {ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});}
