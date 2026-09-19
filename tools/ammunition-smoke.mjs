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
  for (let i = 0; i < 600; i++) {
    if (await evaluate(expression)) return;
    await delay(50);
  }
  await screenshot('ammunition-failure');
  const state=await evaluate('({tick:wasm_exports.comparison_tick(),cursor:wasm_exports.comparison_cursor(),valid:wasm_exports.comparison_valid()})');
  throw new Error(`Timed out: ${label}: ${JSON.stringify(state)}`);
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

const keys=['fracture.object.v1','fracture.experiments.v1'];
let originals;
const ammoStats=()=>evaluate('({ammo:wasm_exports.lab_ammunition(),cut:wasm_exports.lab_ablated(),area:wasm_exports.lab_ablated_area()})');
const saved=async()=> (await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))")).experiments[0];
async function field(x,y,value) {
  await click(x,y);
  for(const ch of value) await cdp('Input.dispatchKeyEvent',{type:'char',text:ch,key:ch});
  await delay(70);await key('Enter','Enter',13);
}
async function trigger(down) {
  await cdp('Input.dispatchKeyEvent',{type:down?'keyDown':'keyUp',code:'KeyF',key:'f',windowsVirtualKeyCode:70});
}
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5','startup');
  originals=await evaluate(`JSON.stringify(${JSON.stringify(keys)}.map(k=>localStorage.getItem(k)))`);
  await evaluate("localStorage.removeItem('fracture.experiments.v1')");
  assert.equal((await ammoStats()).ammo,0);
  await screenshot('ammunition-ready');
  await key('KeyO','o',79);await key('KeyL','l',76);await click(450,502);
  await click(1160,40); // Accepted Wall starter -> TEST.
  await key('Digit6','6',54);assert.equal((await ammoStats()).ammo,1);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});
  await key('KeyF','f',70);
  await until('wasm_exports.lab_explosions()===1','AP impact');
  assert.equal((await stats()).shots,1);
  await screenshot('ammunition-ap');
  await key('Digit7','7',55);await key('KeyR','r',82);
  assert.equal((await ammoStats()).ammo,2);
  let start=await stats();
  await trigger(true);
  await until(`wasm_exports.lab_ticks()>=${start.ticks+120}`,'one second MG burst');
  await trigger(false);await delay(80);
  let gun=await stats();
  assert.ok(gun.shots>=11 && gun.shots<=15,JSON.stringify(gun));
  assert.equal(gun.explosions,0);assert.equal(gun.chunks,1);assert.equal(gun.unconverged,0);
  await delay(250);assert.equal((await stats()).shots,gun.shots,'release stops automatic fire');
  await screenshot('ammunition-mg');
  // MG must visibly damage an actual starter, not only an artificially thin test panel.
  await key('KeyO','o',79);await key('KeyL','l',76);await click(450,302);await click(1160,40);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});
  await trigger(true);await until('wasm_exports.lab_plastic_strain()>0.005','MG dents default barrel');
  await screenshot('ammunition-mg-barrel');await trigger(false);await delay(100);
  assert.equal((await stats()).explosions,0);assert.equal((await stats()).unconverged,0);
  await key('KeyO','o',79);await key('KeyL','l',76);await click(974,302);await click(1160,40);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});
  await trigger(true);await until('wasm_exports.lab_explosions()===1','MG ignites explosive barrel');
  await trigger(false);await delay(100);assert.equal((await stats()).unconverged,0);
  await key('KeyO','o',79);await key('KeyL','l',76);await click(450,502);await click(1160,40);
  await key('Digit8','8',56);await key('KeyR','r',82);
  start=await stats();
  await trigger(true);
  await until(`wasm_exports.lab_ablated()>0 && wasm_exports.lab_ticks()>=${start.ticks+240}`,'laser material removal');
  await screenshot('ammunition-laser');
  await trigger(false);await delay(100);
  const cut=await ammoStats();const laser=await stats();
  assert.ok(cut.area>0 && cut.area<0.04);assert.ok(laser.shots>=12 && laser.shots<=15);assert.equal(laser.explosions,0);assert.equal(laser.shells,0);
  assert.equal(laser.unconverged,0);
  assert.ok(Math.abs(laser.x-start.x)<1e-5 && Math.abs(laser.y-start.y)<1e-5,'laser has no recoil');
  // HUD buttons and cycle key select without firing into the world.
  const shotCount=laser.shots;
  await click(1100,825);assert.equal((await ammoStats()).ammo,0);
  await key('KeyB','b',66);assert.equal((await ammoStats()).ammo,1);
  assert.equal((await stats()).shots,shotCount);
  // Paused automatic weapons queue only a single pulse.
  await key('Digit8','8',56);await key('KeyR','r',82);await key('Space',' ',32);
  await key('KeyF','f',70);assert.equal((await stats()).shots,0);
  await key('KeyN','n',78);assert.equal((await stats()).shots,1);
  await key('KeyN','n',78);assert.equal((await stats()).shots,1);
  // Ammunition and beam checkpoints survive actual comparison save and replay.
  await key('KeyO','o',79);await click(1320,40);
  await until('wasm_exports.comparison_active()===1','comparison');
  await field(235,125,'Ammunition browser trial');
  await click(1240,170);await field(1130,465,'2');
  const selected=await evaluate('wasm_exports.comparison_selected()');
  await click(1100,215); // HE -> AP
  await click(888,42);assert.equal((await saved()).variants[selected].impact.ammunition,'armor_piercing');
  await click(1100,215); // AP -> MG
  await click(888,42);assert.equal((await saved()).variants[selected].impact.shell_mass,.024);
  await click(1100,215); // MG -> laser
  await click(1100,530); // Copy impact to all cases.
  await click(888,42);
  const recipe=await saved();assert.equal(recipe.duration_ticks,240);
  assert.ok(recipe.variants.every(v=>v.impact.ammunition==='laser'));
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await click(610,42);
  await until('wasm_exports.comparison_tick()===240 && wasm_exports.comparison_cursor()===240','laser comparison completes');
  await click(888,42);
  const first=await saved();
  assert.ok(first.variants.every(v=>v.impact.ammunition==='laser'));
  assert.ok(first.review.every(r=>r.frame.beam && r.frame.metrics.ablated_area>0 && r.frame.metrics.unconverged===0));
  await screenshot('ammunition-comparison');
  await click(610,42);
  await until('wasm_exports.comparison_tick()===240 && wasm_exports.comparison_cursor()===240','same ammunition replay');
  await click(888,42);assert.deepEqual((await saved()).review,first.review);
  const six=new URL(url);six.searchParams.set('experiment','barrel-shot');
  await cdp('Page.navigate',{url:six.href});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_cases?.()===6','six-case toolbar');
  await evaluate('window.ammunitionExports=wasm_exports');
  await click(985,190); // Shared HE -> AP
  await click(985,190); // Shared AP -> MG
  await field(810,190,'1');await click(1350,190);
  await until('wasm_exports.comparison_tick()===120','six-case MG burst');
  await click(885,42);
  const library=await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))");
  const mg=library.experiments.find(e=>e.name==='Barrels / shot response / 6 cases');
  assert.ok(mg.variants.every(v=>v.impact.ammunition==='machine_gun' && v.impact.shell_mass===.024 && v.impact.shell_speed===850));
  assert.ok(mg.review.every(r=>r.frame.metrics.broken===0 && r.frame.metrics.unconverged===0));
  assert.equal(await evaluate('window.ammunitionExports===wasm_exports'),true);
  await screenshot('ammunition-six-cases');
  assert.deepEqual(errors,[]);
  console.log('Ammunition browser checks passed: selection, AP, held MG/laser, release, pause, cutting, saved comparison and exact replay.');
} finally {
  if(originals) {
    const values=JSON.parse(originals);
    await evaluate(`${JSON.stringify(keys)}.forEach((k,i)=>{const v=${JSON.stringify(values)}[i];if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)})`).catch(()=>{});
  }
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
