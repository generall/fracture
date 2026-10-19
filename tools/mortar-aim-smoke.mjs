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
let originalMap;
try {
 await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
 await cdp('Network.enable');await cdp('Network.setCacheDisabled',{cacheDisabled:true});
 await cdp('Page.addScriptToEvaluateOnNewDocument',{source:`
  window.pad={connected:true,index:0,mapping:'standard',axes:[0,0,0,0],buttons:Array.from({length:17},()=>({pressed:false,value:0}))};
  Object.defineProperty(navigator,'getGamepads',{value:()=>[pad]});
 `});
 await cdp('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 await cdp('Page.navigate',{url});
 await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden','startup');
 originalMap=await evaluate('localStorage.getItem("fracture.match-map.v1")');
 await click(260,375);await until('wasm_exports.game_screen()===1','lobby');
 await importMap(fixture);await click(1100,705);await until('wasm_exports.game_phase()===2','deployment');
 await click(995,540);await until('wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===2','mortar');
 const range=()=>evaluate('wasm_exports.lab_mortar_range()');
 const initialRange=await range();
 await evaluate('pad.axes[3]=-0.4');await delay(350);
 const fineRange=await range();
 assert.ok(fineRange>initialRange && fineRange<initialRange+0.4,'a small stick touch adjusts range finely');
 await evaluate('pad.axes[3]=0');await delay(100);
 const heldRange=await range();await delay(250);
 assert.equal(await range(),heldRange,'range is retained on release');
 await evaluate('pad.axes[2]=0.7');await delay(250);
 assert.equal(await range(),heldRange,'horizontal movement cannot reset range');
 await evaluate('pad.axes[2]=0;pad.axes[3]=-1');await delay(550);
 const farther=await range();
 assert.ok(farther>heldRange+3 && farther<heldRange+8 && farther<36,'full up adjusts range over time');
 await evaluate('pad.axes[3]=1');await delay(450);
 assert.ok(await range()<farther-2,'stick down shortens the range');
 await evaluate('pad.axes[3]=0');
 await screenshot('mortar-stick-fine-range');
 await hold('keyDown','KeyQ','q',81);await hold('keyDown','KeyE','e',69);await delay(100);await hold('keyUp','KeyQ','q',81);await hold('keyUp','KeyE','e',69);
 await delay(4000);
 const spread=()=>evaluate('wasm_exports.lab_aim_spread()');
 const steady=await spread();assert.ok(steady<Math.PI/180,'precise while stationary');
 await screenshot('mortar-aim-steady');
 await hold('keyDown','KeyW','w',87);await delay(6000);
 const moving=await spread();assert.ok(moving>8*Math.PI/180,'moving widens landing circle');
 await screenshot('mortar-aim-moving');
 await hold('keyUp','KeyW','w',87);await delay(6500);
 const settled=await spread();assert.ok(settled<Math.PI/180,'stopping restores precise aim');
 await screenshot('mortar-aim-settled');
 await hold('keyDown','KeyD','d',68);await delay(1800);
 const turning=await spread();assert.ok(turning>2*Math.PI/180 && turning>steady*3,`slow pivot also widens landing circle: ${turning}`);
 await screenshot('mortar-aim-turning');
 await hold('keyUp','KeyD','d',68);
 assert.equal(errors.length,0,JSON.stringify(errors));
 console.log(`PASS: gradual mortar joystick range, release hold, horizontal stability; spread ${steady.toFixed(4)} stationary → ${moving.toFixed(4)} moving → ${settled.toFixed(4)} settled; ${turning.toFixed(4)} while turning`);
} catch(e) {await screenshot('mortar-aim-error');throw e;}
finally {
 if(originalMap!==undefined) await evaluate(originalMap===null?'localStorage.removeItem("fracture.match-map.v1")':`localStorage.setItem("fracture.match-map.v1",${JSON.stringify(originalMap)})`);
 ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
