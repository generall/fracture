// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
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

let originalStores;
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1','game startup');
  originalStores=await evaluate('Object.fromEntries(["fracture.level.v1","fracture.match-map.v1"].map(k=>[k,localStorage.getItem(k)]))');
  assert.equal(await evaluate('wasm_exports.game_screen()'),0);
  await until('document.getElementById("status").hidden','loading screen hidden');
  await screenshot('game-home');
  await click(260,512);
  await until('wasm_exports.game_screen()===1','lobby');
  await screenshot('game-lobby');
  await click(170,489);
  await until('wasm_exports.game_screen()===2 && wasm_exports.level_editor_active()===1','map builder');
  await click(75,525); await mapClick(0,4);
  await screenshot('game-map-builder');
  await click(1135,40);
  await until('wasm_exports.game_screen()===1','use map returns to lobby');
  const created=await evaluate('JSON.parse(localStorage.getItem("fracture.match-map.v1"))');
  assert.ok(created.items.length>4,'builder adds an actual object');
  assert.equal(await evaluate('localStorage.getItem("fracture.level.v1")'),originalStores['fracture.level.v1'],'sandbox draft preserved');
  // Choose red and change the time/opponent settings before starting.
  await click(220,714);
  await click(610,714);
  await click(980,714);
  await click(1250,837);
  await until('wasm_exports.game_phase()===1','countdown');
  assert.equal(await evaluate('wasm_exports.game_match_ticks()'),0);
  assert.ok((await stats()).x>0,'red spawn selected');
  await key('Space',' ',32);
  assert.equal((await stats()).shots,0,'countdown blocks firing');
  await screenshot('game-countdown');
  await until('wasm_exports.game_phase()===2 && wasm_exports.game_match_ticks()>10','match running');
  const tickBefore=(await stats()).ticks;
  for(const [code,name,n] of [['KeyM','m',77],['KeyO','o',79],['KeyR','r',82],['Digit1','1',49],['KeyN','n',78],['KeyT','t',84],['KeyV','v',86]]) await key(code,name,n);
  assert.equal(await evaluate('wasm_exports.game_screen()'),3,'sandbox shortcuts cannot leave the match');
  assert.equal(await evaluate('wasm_exports.level_editor_active()'),0);
  assert.ok((await stats()).ticks>tickBefore,'reset shortcut cannot rebuild the match');
  await key('Space',' ',32);
  assert.ok((await stats()).shots>0,'Space fires the main gun in a match');
  assert.equal(await evaluate('wasm_exports.game_phase()'),2,'Space does not open the pause menu');
  await screenshot('game-match');
  await key('Escape','Escape',27);
  await until('wasm_exports.game_phase()===3','pause menu');
  const frozen=await evaluate('wasm_exports.game_match_ticks()');
  await key('KeyN','n',78); await key('Space',' ',32); await delay(200);
  assert.equal(await evaluate('wasm_exports.game_match_ticks()'),frozen,'menu freezes match clock and step is disabled');
  await screenshot('game-pause');
  await click(710,448);
  await until('wasm_exports.game_phase()===2','resume');
  await until(`wasm_exports.game_match_ticks()>${frozen}`,'clock resumes');
  await key('Escape','Escape',27);
  await click(715,550);
  await until('wasm_exports.game_screen()===1','return to lobby');
  const quick={version:1,name:'Flag capture check',size:[48,30],enemy_activity:'static',items:[
    {kind:'spawn',team:'blue',position:[-18,0],angle:0},
    {kind:'spawn',team:'red',position:[18,0],angle:Math.PI},
    {kind:'flag',team:'blue',position:[-21,0]},
    {kind:'flag',team:'red',position:[-14,0]}
  ]};
  const invalid=structuredClone(quick); invalid.items.pop();
  await importMap(invalid);
  await click(1250,837); await delay(200);
  assert.equal(await evaluate('wasm_exports.game_screen()'),1,'missing flag blocks start without falling back to sandbox');
  await screenshot('game-invalid-map');
  await importMap(quick);
  await click(220,714); // Back to blue.
  await click(1250,837);
  await until('wasm_exports.game_phase()===2','capture match begins');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.game_phase()===4','match results');
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  assert.equal(await evaluate('wasm_exports.lab_winner()'),1);
  const ended=await evaluate('wasm_exports.game_match_ticks()');
  await delay(180); assert.equal(await evaluate('wasm_exports.game_match_ticks()'),ended);
  await screenshot('game-results');
  await click(560,565);
  await until('wasm_exports.game_phase()===1','rematch countdown');
  assert.equal(await evaluate('wasm_exports.game_match_ticks()'),0);
  assert.equal((await stats()).shots,0);
  await key('Escape','Escape',27); await click(715,550);
  await until('wasm_exports.game_screen()===1','leave rematch during countdown');
  const defeat=structuredClone(quick); defeat.name='Defeat check';
  defeat.items[2].position=[16,0]; defeat.items[3].position=[21,0];
  await importMap(defeat); await click(1250,837);
  await until('wasm_exports.game_phase()===4','AI victory produces results');
  assert.equal(await evaluate('wasm_exports.lab_winner()'),2);
  await screenshot('game-defeat');
  await click(850,565);
  await until('wasm_exports.game_screen()===1','leave defeat');
  await importMap(quick);
  await cdp('Page.reload');
  await until('wasm_exports?.game_ready?.()===1 && wasm_exports.game_screen()===0','reload to home');
  await click(260,512); await click(185,407);
  await screenshot('game-saved-map');
  assert.equal(await evaluate('JSON.parse(localStorage.getItem("fracture.match-map.v1")).name'),quick.name);
  assert.equal(await evaluate('localStorage.getItem("fracture.level.v1")'),originalStores['fracture.level.v1']);
  await key('Escape','Escape',27);
  await click(1200,833);
  assert.equal(await evaluate('wasm_exports.game_screen()'),0,'game has no sandbox switch');
  const sandboxPage = new URL('sandbox.html', url);
  await cdp('Page.navigate', {url: sandboxPage.href});
  await until('typeof wasm_exports!=="undefined" && wasm_exports.game_screen?.()===4 && wasm_exports.lab_ticks()>2','separate sandbox application');
  await key('Escape','Escape',27);
  assert.equal(await evaluate('wasm_exports.game_screen()'),4,'sandbox cannot return to game title');
  await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','sandbox editor still works');
  assert.deepEqual(errors,[]);
  console.log('PASS: home, lobby, map creation/persistence, config, countdown, gameplay, sandbox isolation, pause, validation, capture, results, rematch, saved-map reload and sandbox access');
} finally {
  if(originalStores) await evaluate(`Object.entries(${JSON.stringify(originalStores)}).forEach(([k,v])=>v===null?localStorage.removeItem(k):localStorage.setItem(k,v))`);
  await fetch(`${debugUrl}/json/close/${page.id}`); ws.close();
}
