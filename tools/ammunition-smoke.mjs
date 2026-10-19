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

const map = () => evaluate('JSON.parse(localStorage.getItem("fracture.level.v1"))');
const mapCamera = () => evaluate('({scale:wasm_exports.level_editor_scale(),x:wasm_exports.level_editor_origin_x(),y:wasm_exports.level_editor_origin_y()})');
async function at(x,y) {const c=await mapCamera();return [c.x+x*c.scale,c.y+y*c.scale];}
async function mapClick(x,y) {await click(...await at(x,y));}
async function dragPoints(a,b,modifiers=0,cancel=false) {
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:a[0],y:a[1]});await delay(30);
  await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:a[0],y:a[1],button:'left',clickCount:1,modifiers});await delay(35);
  for(let i=1;i<=8;i++){await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:a[0]+(b[0]-a[0])*i/8,y:a[1]+(b[1]-a[1])*i/8,buttons:1,modifiers});await delay(30);}
  if(cancel) await key('Escape','Escape',27);
  await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:b[0],y:b[1],button:'left',clickCount:1,modifiers});await delay(150);
}
async function mapDrag(a,b){await dragPoints(await at(...a),await at(...b));}
async function field(x,y,value){await click(x,y);for(const ch of String(value)) await cdp('Input.dispatchKeyEvent',{type:'char',text:ch,key:ch});await delay(80);await key('Enter','Enter',13);}
async function importMap(level){const json=JSON.stringify(level);await evaluate(`(()=>{const bytes=new TextEncoder().encode(${JSON.stringify(json)});const ptr=wasm_exports.level_import_buffer(bytes.length);new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);wasm_exports.level_import_commit();})()`);await delay(200);}

let originalLevel;
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5','startup');
  originalLevel=await evaluate('localStorage.getItem("fracture.level.v1")');
  await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','editor');
  const level={version:1,name:'Special weapons check',size:[32,24],enemy_activity:'static',items:[
    {kind:'spawn',team:'blue',position:[-8,0],angle:0},
    {kind:'spawn',team:'red',position:[8,4],angle:Math.PI},
    {kind:'flag',team:'blue',position:[-13,0]},
    {kind:'flag',team:'red',position:[13,0]},
    {kind:'pickup',weapon:'laser',position:[-8,0],respawn_time:20},
    {kind:'pickup',weapon:'rocket',position:[-4,3],respawn_time:20}
  ]};
  await importMap(level);
  await click(225,201); await click(80,150); await mapClick(-8,0);
  await field(1260,454,12);
  assert.equal((await map()).items[4].respawn_time,12);
  await screenshot('special-pickup-editor');
  await click(1135,40);
  await until('wasm_exports.lab_special()===1','laser collected');
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1090,y:450});
  await delay(300);
  await key('KeyG','g',71);
  await until('wasm_exports.lab_laser_remaining()>1','laser started');
  assert.equal(await evaluate('wasm_exports.lab_special()'),0);
  await delay(450);
  await key('KeyP','p',80);
  await screenshot('special-laser-firing');
  const frozen=await evaluate('wasm_exports.lab_laser_remaining()');
  await delay(150); assert.equal(await evaluate('wasm_exports.lab_laser_remaining()'),frozen);
  await key('KeyP','p',80);
  await until('wasm_exports.lab_laser_remaining()===0','laser finished after release');
  await key('Digit6','6',54);
  assert.equal(await evaluate('wasm_exports.lab_ammunition()'),1);
  assert.ok(await evaluate('wasm_exports.lab_reload()>0.5'));
  await until('wasm_exports.lab_reload()===0','AP loaded');
  await key('Digit5','5',53);
  assert.ok(await evaluate('wasm_exports.lab_reload()>0.5'));
  await key('Digit7','7',55);
  assert.equal(await evaluate('wasm_exports.lab_ammunition()'),0,'removed MG key cannot select special');
  await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','return to editor');
  // Use the actual palette button, then verify the authored pad survives JSON import.
  await click(225,201);
  await click(225,623);
  await mapClick(0,-4);
  assert.ok((await map()).items.some(i=>i.kind==='pickup' && i.weapon==='plasma'));
  await screenshot('special-plasma-editor');
  const plasmaLevel=structuredClone(level);
  plasmaLevel.items[4].weapon='plasma';
  plasmaLevel.items[4].respawn_time=20;
  await importMap(plasmaLevel);
  await click(1135,40);
  await until('wasm_exports.lab_special()===3','plasma collected');
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1090,y:450});
  await key('KeyP','p',80);
  const beforePlasma=await stats();
  await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:1090,y:450,button:'right',clickCount:1});
  await delay(30);
  await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:1090,y:450,button:'right',clickCount:1});
  assert.equal(await evaluate('wasm_exports.lab_plasma_remaining()'),0,'paused activation waits for a tick');
  await key('KeyN','n',78);
  assert.equal(await evaluate('wasm_exports.lab_special()'),0,'one charge consumed');
  const charge=await evaluate('wasm_exports.lab_plasma_remaining()');
  assert.ok(charge>0.98 && charge<1,'one simulated tick of charge');
  assert.equal((await stats()).shots,beforePlasma.shots,'no immediate shot');
  await screenshot('special-plasma-charging');
  await delay(150);
  assert.equal(await evaluate('wasm_exports.lab_plasma_remaining()'),charge,'pause freezes charge');
  for(let i=0;i<118;i++) await key('KeyN','n',78);
  assert.equal((await stats()).shots,beforePlasma.shots,'no shot before one second');
  await key('KeyN','n',78);
  assert.equal(await evaluate('wasm_exports.lab_plasma_remaining()'),0);
  assert.equal((await stats()).shots,beforePlasma.shots+1,'single discharge at one second');
  await screenshot('special-plasma-pulse');
  await key('KeyP','p',80);
  await delay(500);
  assert.equal((await stats()).shots,beforePlasma.shots+1,'no sustained fire');
  await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','return for mine placement');
  await click(225,201);
  await click(195,668);
  await mapClick(0,3);
  assert.ok((await map()).items.some(i=>i.kind==='pickup' && i.weapon==='mine'),'mine palette authors a pickup');
  await screenshot('special-mine-editor');
  const mineLevel=structuredClone(level);
  mineLevel.items[4].weapon='mine';
  mineLevel.items[4].charges=2;
  mineLevel.items[4].cooldown=0.3;
  await importMap(mineLevel);
  await click(1135,40);
  await until('wasm_exports.lab_special()===4','mines collected');
  assert.equal(await evaluate('wasm_exports.lab_mines()'),0);
  const beforeMines=await stats();
  await key('KeyG','g',71);
  await until('wasm_exports.lab_mines()===1','first mine placed');
  assert.equal(await evaluate('wasm_exports.lab_special()'),4,'one mine charge remains');
  await delay(400);
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await delay(450);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await key('KeyG','g',71);
  await until('wasm_exports.lab_mines()===2','second mine placed while moving');
  assert.equal(await evaluate('wasm_exports.lab_special()'),0,'last mine empties inventory');
  assert.equal((await stats()).shots,beforeMines.shots,'placing mines does not fire the gun');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until(`wasm_exports.lab_tank_x()>${beforeMines.x+3}`,'drive clear of the deployed mines');
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  assert.equal(await evaluate('wasm_exports.lab_tank_alive()'),1,'friendly mines are safe after arming');
  await screenshot('special-mines-deployed');
  assert.deepEqual(errors,[]);
  console.log('PASS: pickup editor, laser, HE/AP reload, plasma charge/pulse, mine palette, mine collection, movement/placement, charges and friendly safety');
} finally {
  if (originalLevel !== undefined) {
    await evaluate(originalLevel === null ? 'localStorage.removeItem("fracture.level.v1")' : `localStorage.setItem("fracture.level.v1",${JSON.stringify(originalLevel)})`);
  }
  await fetch(`${debugUrl}/json/close/${page.id}`);
  ws.close();
}
