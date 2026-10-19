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
const fixture={version:1,name:'Vehicle deployment check',size:[48,30],enemy_activity:'static',items:[
 {kind:'spawn',team:'blue',position:[-18,0],angle:0},
 {kind:'spawn',team:'red',position:[18,0],angle:Math.PI},
 {kind:'flag',team:'blue',position:[-21,0]}, {kind:'flag',team:'red',position:[21,0]},
 {kind:'terrain',terrain:'Water',min:[-2,-14.7],max:[2,14.7]}
]};
try {
 await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
 await cdp('Network.enable');await cdp('Network.setCacheDisabled',{cacheDisabled:true});
 await cdp('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 for (let kind=0;kind<3;kind++) {
  await cdp('Page.navigate',{url});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden','startup');
  await click(260,375);await until('wasm_exports.game_screen()===1','lobby');
  await importMap(fixture);await click(1100,705);
  await until('wasm_exports.game_phase()===2','deployment');
  assert.equal(await evaluate('wasm_exports.lab_tank_alive()'),0);
  if(kind===0)await screenshot('vehicle-deployment-deck');
  await click(285+kind*355,540);
  await until(`wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===${kind}`,'selected vehicle spawns');
  await screenshot(['vehicle-scout','vehicle-tank','vehicle-mortar'][kind]);
  if(kind===0){await hold('keyDown','Space',' ',32);await delay(1300);await hold('keyUp','Space',' ',32);assert.ok(await evaluate('wasm_exports.lab_shots()')>=4,'automatic scout gun');}
  if(kind===2){await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:700,y:200});await delay(1400);assert.ok(Math.abs(await evaluate('wasm_exports.lab_turret_angle()'))<0.01,'mortar gun fixed to hull');}
 }
 await cdp('Page.navigate',{url});await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden','new game');
 await click(260,375);await until('wasm_exports.game_screen()===1','lobby');
 const combat=structuredClone(fixture);combat.name='Death and deployment check';
 combat.items.push({kind:'turret',team:'red',position:[-8,0],angle:Math.PI,arc:120,capturable:true});
 await importMap(combat);assert.equal(await evaluate('JSON.parse(localStorage.getItem("fracture.match-map.v1")).name'),combat.name);await click(1100,705);await until('wasm_exports.game_phase()===2','deployment');
 await click(285,540);await until('wasm_exports.lab_tank_alive()===1','scout');
 await until('wasm_exports.lab_tank_alive()===0','turret destroys scout',400);
 await delay(3400);assert.equal(await evaluate('wasm_exports.lab_tank_alive()'),0,'no automatic respawn');
 await screenshot('vehicle-respawn-choice');
 await click(995,540);await until('wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===2','choose mortar after death');
 await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
 await cdp('Page.navigate',{url:new URL('sandbox.html',url).href});
 await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5 && document.getElementById("status").hidden','sandbox');
 for (let kind=0;kind<3;kind++) {
  const ticks=await evaluate('wasm_exports.lab_ticks()');
  await key('KeyU','u',85);await until('wasm_exports.lab_tank_alive()===0','sandbox picker');
  await click(365+kind*355,590);
  await until(`wasm_exports.lab_tank_alive()===1 && wasm_exports.lab_vehicle()===${kind}`,'sandbox vehicle deploy');
  assert.ok(await evaluate('wasm_exports.lab_ticks()')>ticks, 'sandbox scene continues without rebuilding');
 }
 await screenshot('sandbox-mortar');
 await key('KeyM','m',77);await until('wasm_exports.level_editor_active()===1','editor');
 await importMap({...fixture,name:'Turret editor check',items:fixture.items.filter(i=>i.kind!=='terrain')});
 await click(220,201);await click(145,668);
 const view=await evaluate('({x:wasm_exports.level_editor_origin_x(),y:wasm_exports.level_editor_origin_y(),s:wasm_exports.level_editor_scale()})');
 await click(view.x,view.y);await delay(300);
 await until('JSON.parse(localStorage.getItem("fracture.level.v1") || "null")?.items.some(i=>i.kind==="turret")','place turret');
 await click(1280,310);await click(1280,455);
 await until('JSON.parse(localStorage.getItem("fracture.level.v1")).items.some(i=>i.kind==="turret" && i.team==="red" && i.capturable===false)', 'turret ownership and capture toggle');
 for(const [x,y,value] of [[1280,380,'45'],[1220,526,'90'],[1355,526,'750']]) {await click(x,y);for(const ch of value)await cdp('Input.dispatchKeyEvent',{type:'char',text:ch,key:ch});await key('Enter','Enter',13);}
 await until('JSON.parse(localStorage.getItem("fracture.level.v1")).items.some(i=>i.kind==="turret" && i.arc===90 && Math.abs(i.angle-Math.PI/4)<0.001)', 'turret rotation and arc');
 await click(1280,310);
 await until('JSON.parse(localStorage.getItem("fracture.level.v1")).items.some(i=>i.kind==="turret" && i.team==="neutral" && i.health===750)', 'neutral turret and custom health');
 await screenshot('turret-editor');
 assert.equal(errors.length,0,JSON.stringify(errors));console.log('PASS: three vehicle designs, deployment, automatic scout gun, fixed mortar, turret kill, explicit respawn choice, turret editor placement');
}catch(e){await screenshot('vehicles-turrets-error');console.log(await evaluate('({phase:wasm_exports.game_phase(),alive:wasm_exports.lab_tank_alive(),ticks:wasm_exports.lab_ticks(),shots:wasm_exports.lab_shots(),broken:wasm_exports.lab_broken()})'));throw e;}finally{ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});}
