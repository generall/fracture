// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --mute-audio --remote-debugging-port=9227.
// Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.GAME_URL || 'http://127.0.0.1:8000');
sandboxUrl.searchParams.delete('mode');
sandboxUrl.searchParams.set('deck','1');
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

const fixture={version:1,name:'Camera aim check',size:[160,160],enemy_activity:'static',items:[
 {kind:'spawn',team:'blue',position:[-50,0],angle:0},
 {kind:'spawn',team:'red',position:[50,0],angle:Math.PI},
 {kind:'flag',team:'blue',position:[-65,0]}, {kind:'flag',team:'red',position:[65,0]}
]};
const state=()=>evaluate('({x:wasm_exports.lab_tank_x(),y:wasm_exports.lab_tank_y(),cx:wasm_exports.lab_camera_x(),cy:wasm_exports.lab_camera_y(),z:wasm_exports.lab_zoom(),angle:wasm_exports.lab_turret_angle()})');
async function framing(height,label) {
 const s=await state();
 const scale=Math.min((1280-90)/24,(height-95)/15)*s.z;
 const dx=Math.cos(s.angle),dy=Math.sin(s.angle),ox=s.cx-s.x,oy=s.cy-s.y;
 assert.ok(ox*dx+oy*dy>1,`${label}: camera looks along actual barrel`);
 assert.ok(Math.abs(ox*dy-oy*dx)<0.5,`${label}: camera follows the aim direction`);
 const centeredReach=Math.min(640/scale/Math.max(Math.abs(dx),0.0001),height/2/scale/Math.max(Math.abs(dy),0.0001));
 const target=[640+(s.x+dx*centeredReach*1.4-s.cx)*scale,height/2+(s.y+dy*centeredReach*1.4-s.cy)*scale];
 assert.ok(target[0]>0 && target[0]<1280 && target[1]>0 && target[1]<height,`${label}: reveals at least 40% more range ahead`);
 for(const corner of [-1.8,1.8]) {
  const px=640+(s.x+corner-s.cx)*scale,py=height/2+(s.y+corner-s.cy)*scale;
  assert.ok(px>0 && px<1280 && py>0 && py<height,`${label}: vehicle remains visible`);
 }
 return s;
}
let originalMap;
try {
 await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
 await cdp('Network.enable');await cdp('Network.setCacheDisabled',{cacheDisabled:true});
 await cdp('Page.addScriptToEvaluateOnNewDocument',{source:`
  window.pad={connected:true,index:0,mapping:'standard',axes:[0,0,0,0],buttons:Array.from({length:17},()=>({pressed:false,value:0}))};
  Object.defineProperty(navigator,'getGamepads',{value:()=>[pad]});
 `});
 for(const [kind,height] of [[1,800],[1,720],[0,800],[2,800]]) {
  await cdp('Emulation.setDeviceMetricsOverride',{width:1280,height,deviceScaleFactor:1,mobile:false});
  await cdp('Page.navigate',{url});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden','startup');
  if(originalMap===undefined) originalMap=await evaluate('localStorage.getItem("fracture.match-map.v1")');
  await click(260,375);await until('wasm_exports.game_screen()===1','lobby');
  await importMap(fixture);await click(1100,height-95);await until('wasm_exports.game_phase()===2','deployment');
  await click(285+kind*355,height/2+140);
  await until(`wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===${kind}`,'vehicle deployed');
  const directions=kind===2?[[0,-0.4]]:[[0,-1],[1,0],[0,1],[-1,0]];
  for(const [x,y] of directions) {
   await evaluate(`pad.axes[2]=${x};pad.axes[3]=${y}`);await delay(3200);
   const aimed=await framing(height,`vehicle ${kind}, stick ${x},${y}, ${height}p`);
   await evaluate('pad.axes[2]=0;pad.axes[3]=0');
   await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1100,y:40});await delay(500);
   const released=await state();
   assert.equal(await evaluate('wasm_exports.controller_active()'),1);
   assert.ok(Math.abs(released.angle-aimed.angle)<0.03,'release and emulated mouse retain aim');
   assert.ok(Math.hypot(released.cx-aimed.cx,released.cy-aimed.cy)<0.4,'release retains camera direction');
  }
  if(kind===2) {
   const fixed=await state();
   assert.ok(fixed.cx>fixed.x+1 && Math.abs(fixed.cy-fixed.y)<0.2,'mortar range stick does not redirect camera away from hull');
   const range=await evaluate('wasm_exports.lab_mortar_range()');
   const shots=await evaluate('wasm_exports.lab_shots()');
   // Steam's desktop mappings can deliver a delayed mouse click or fire key
   // in addition to RT. Exercise these after stick release, when no axis input
   // is available to reclaim controller ownership.
   await evaluate('pad.buttons[7]={pressed:true,value:1}');await delay(100);
   await click(1250,30);await delay(100);
   await evaluate('pad.buttons[7]={pressed:false,value:0}');await delay(100);
   await click(1250,30);
   for(const [code,key,keyCode] of [['Space',' ',32],['KeyF','f',70],['KeyG','g',71]]) {
    await cdp('Input.dispatchKeyEvent',{type:'keyDown',code,key,windowsVirtualKeyCode:keyCode});await delay(80);
    await cdp('Input.dispatchKeyEvent',{type:'keyUp',code,key,windowsVirtualKeyCode:keyCode});await delay(80);
   }
   assert.ok(await evaluate('wasm_exports.lab_shots()')>shots,'RT fires the mortar');
   assert.equal(await evaluate('wasm_exports.controller_active()'),1,'fire aliases cannot activate stale cursor');
   assert.equal(await evaluate("getComputedStyle(document.getElementById('glcanvas')).cursor"),'none','cursor stays hidden through firing');
   assert.equal(await evaluate('wasm_exports.lab_mortar_range()'),range,'firing retains selected range');
   const markerRange=await evaluate('Math.hypot(wasm_exports.lab_aim_x()-wasm_exports.lab_tank_x(),wasm_exports.lab_aim_y()-wasm_exports.lab_tank_y())');
   assert.ok(Math.abs(markerRange-range)<0.02,'marker retains the selected firing range after recoil and emulated inputs');
   await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:800,y:400,button:'middle',clickCount:1});await delay(80);
   await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:800,y:400,button:'middle',clickCount:1});await delay(80);
   assert.equal(await evaluate('wasm_exports.controller_active()'),0,'deliberate middle click still restores pointer control');
   console.log('PASS: mortar firing, recoil and emulated fire inputs retain range and keep cursor hidden');
  }
  await screenshot(`camera-aim-vehicle-${kind}-${height}`);
  console.log(`PASS: aim look-ahead, release hold and vehicle visibility: vehicle ${kind}, 1280×${height}`);
 }
 assert.equal(errors.length,0,JSON.stringify(errors));
} catch(e) {await screenshot('camera-aim-error');throw e;}
finally {
 if(originalMap!==undefined) await evaluate(originalMap===null?'localStorage.removeItem("fracture.match-map.v1")':`localStorage.setItem("fracture.match-map.v1",${JSON.stringify(originalMap)})`);
 ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
