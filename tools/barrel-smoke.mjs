// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile, readFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.SANDBOX_URL || 'http://127.0.0.1:8000/sandbox.html');
sandboxUrl.searchParams.delete('mode');
const url = sandboxUrl.href;
const demo = new URL(url); demo.searchParams.set('experiment','barrels');
const page = await fetch(`${debugUrl}/json/new?${encodeURIComponent(demo.href)}`, { method: 'PUT' }).then(r => r.json());
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

let originalDraft,originalLibrary;
const archive=()=>evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))");
const saved=async()=> (await archive()).experiments.find(e=>e.name==='Barrels / permanent dents / 10 cases');
async function seek(tick) {await click(28+1384*tick/480,817);await delay(60);}
function barrelShape(state) {
  const geometry=new Map(state.geometry.map(g=>[g.id,g]));
  const points=[];let perimeter=0;
  for(const pose of state.frame.poses) {
    const g=geometry.get(pose[0]);if(g.material!==2)continue;
    const sin=Math.sin(pose[2]),cos=Math.cos(pose[2]);
    const vertices=g.vertices.slice(0,g.count).map(([x,y])=>[cos*x-sin*y+pose[1][0],sin*x+cos*y+pose[1][1]]);
    points.push(...vertices);
    for(let i=0;i<g.count;i++)if(pose[3]&(1<<i)) {
      const a=vertices[i],b=vertices[(i+1)%g.count];perimeter+=Math.hypot(a[0]-b[0],a[1]-b[1]);
    }
  }
  return {width:Math.max(...points.map(p=>p[0]))-Math.min(...points.map(p=>p[0])),height:Math.max(...points.map(p=>p[1]))-Math.min(...points.map(p=>p[1])),perimeter};
}
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_cases?.()===10','ten-case startup');
  originalDraft=await evaluate("localStorage.getItem('fracture.object.v1')");
  originalLibrary=await evaluate("localStorage.getItem('fracture.experiments.v1')");
  await evaluate("localStorage.removeItem('fracture.experiments.v1')");
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_tick?.()===480','all ten recordings');
  assert.equal(await evaluate("document.getElementById('status').hidden"),true);
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await click(700,42);await click(800,42);await screenshot('barrels-start');
  await seek(240);await screenshot('barrels-ten');
  // Last card must be on-screen and selectable in the two-row grid.
  await click(1270,550);
  assert.equal(await evaluate('wasm_exports.comparison_selected()'),9);
  await click(1240,124);assert.equal(await evaluate('wasm_exports.comparison_favorite()'),10);
  await click(885,42);
  const first=await saved();assert.equal(first.variants.length,10);assert.equal(first.favorite,9);
  assert.ok(first.variants.every(v=>v.impact.backstop&&v.impact.kind==='tank'&&v.impact.tank_speed===0.8));
  assert.equal(first.review.length,10);assert.ok(first.review.every(r=>r.frame.tick===first.cursor_tick));
  assert.ok(first.review.every(r=>r.frame.metrics.unconverged===0));
  assert.ok(first.review.every(r=>r.frame.metrics.broken===0));
  assert.ok(first.review.some(r=>r.frame.metrics.plastic_work>0 && r.frame.metrics.plastic_strain>0));
  // Rewinding restores undented geometry; later shape revisions must not leak backward.
  await seek(0);await click(885,42);const initial=await saved();
  assert.ok(initial.review.every(r=>r.frame.metrics.plastic_work===0));
  assert.notDeepEqual(initial.review[0].geometry,first.review[0].geometry);
  for(let i=0;i<10;i++) {
    const before=barrelShape(initial.review[i]),after=barrelShape(first.review[i]);
    assert.ok(after.width<before.width-0.03,`barrel ${i+1} must compress along the force`);
    assert.ok(after.height>before.height+0.03,`barrel ${i+1} must expand across the force`);
    assert.ok(Math.abs(after.perimeter/before.perimeter-1)<0.0001,`barrel ${i+1} must retain its rim length`);
  }
  await seek(first.cursor_tick);await click(885,42);
  await click(580,124);await screenshot('barrels-focus');
  assert.equal(await evaluate('wasm_exports.comparison_selected()'),9);
  await click(580,124);await click(620,42);
  await until('wasm_exports.comparison_tick()===480','replay');await click(700,42);await seek(first.cursor_tick);
  await click(885,42);const replay=await saved();assert.deepEqual(replay.review,first.review);
  // Restore a saved checkpoint, with ten-case rendering and favorite intact.
  await click(1150,42);await click(600,220);
  assert.equal(await evaluate('wasm_exports.comparison_cases()'),10);
  assert.equal(await evaluate('wasm_exports.comparison_favorite()'),10);
  // Focus -> edit selected, keeping precisely the selected profile.
  await click(580,124);await click(1230,826);
  await until('wasm_exports.editor_active()===1','selected barrel in editor');
  const object=await evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
  assert.equal(object.name,'Barrel 10');assert.equal(object.components.plasticity.yield_stress,8000);assert.equal(object.components.plasticity.hardening,4);
  // USE CASE copies the object; COMPARE starts a new trial from that copy.
  // The original ten-case study remains available in the saved library.
  await click(1320,42);await until('wasm_exports.comparison_cases()===4','compare copied object');
  await click(1150,42);await click(600,220);
  await until('wasm_exports.comparison_cases()===10','restore original ten cases');
  await click(620,42);
  await until('wasm_exports.comparison_tick()===480','record returning experiment');
  await click(700,42);await seek(240);
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});
  await delay(200);await screenshot('barrels-hidpi');
  assert.deepEqual(errors,[]);
  console.log('PASS: ten visible cases, pinned fixture, loop/pause/scrub, focus, favorite, save/reload, exact WASM replay, selected editor profile, return to original experiment, high DPI.');
} finally {
  if(originalDraft!==undefined)await evaluate(originalDraft===null?"localStorage.removeItem('fracture.object.v1')":`localStorage.setItem('fracture.object.v1',${JSON.stringify(originalDraft)})`).catch(()=>{});
  if(originalLibrary!==undefined)await evaluate(originalLibrary===null?"localStorage.removeItem('fracture.experiments.v1')":`localStorage.setItem('fracture.experiments.v1',${JSON.stringify(originalLibrary)})`).catch(()=>{});
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
