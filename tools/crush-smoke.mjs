// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.SANDBOX_URL || 'http://127.0.0.1:8000/sandbox.html');
sandboxUrl.searchParams.delete('mode');
const url = sandboxUrl.href;
const demo = new URL(url); demo.searchParams.set('experiment','crush');
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


let originalDraft, originalLibrary;
const saved = async () => (await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))")).experiments.find(e => e.name === 'Barrels / deformation limit / 6 cases');
async function seek(tick, duration=960) { await click(28 + 1384 * tick / duration, 817); }
async function field(x,y,value) {
  await click(x,y);
  for(const ch of value) await cdp('Input.dispatchKeyEvent',{type:'char',text:ch,key:ch});
  await delay(70); await key('Enter','Enter',13);
}
function sameImpact(e) {
  assert.equal(e.shared_impact,true);
  assert.ok(e.variants.every(v=>JSON.stringify(v.impact)===JSON.stringify(e.variants[0].impact)));
}
try {
  await mkdir('target/screenshots', {recursive: true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride', {width:1440, height:900, deviceScaleFactor:1, mobile:false});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_cases?.()===6', 'six-case startup');
  originalDraft = await evaluate("localStorage.getItem('fracture.object.v1')");
  originalLibrary = await evaluate("localStorage.getItem('fracture.experiments.v1')");
  await evaluate("localStorage.removeItem('fracture.experiments.v1')");
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_tick?.()===960', 'eight-second recordings');
  await evaluate('window.crushTestExports = wasm_exports');
  assert.equal(await evaluate("document.getElementById('status').hidden"), true);
  assert.equal(await evaluate('wasm_exports.comparison_valid()'), 1);
  await click(700,42); await seek(0); await click(885,42);
  const initial = await saved();
  assert.equal(initial.variants.length,6); sameImpact(initial);
  assert.deepEqual(initial.variants.map(v=>v.object.components.plasticity.max_strain),[0.4,0.8,1.2,1.6,2.2,3]);
  const objects=initial.variants.map(v=>v.object);
  const normalized=structuredClone(objects);
  for(const o of normalized)o.components.plasticity.max_strain=0.4;
  assert.ok(normalized.every(o=>JSON.stringify(o)===JSON.stringify(normalized[0])));
  assert.ok(initial.review.every(r=>r.frame.tick===0 && r.frame.metrics.plastic_work===0));
  await screenshot('crush-start');
  await seek(960); await click(1230,550); await click(1240,124); await click(885,42);
  const result=await saved(); assert.equal(result.favorite,5);
  for(let i=0;i<6;i++) {
    const state=result.review[i],before=barrelShape(initial.review[i]),after=barrelShape(state);
    assert.equal(state.frame.tick,960);
    assert.equal(state.frame.metrics.broken,0); assert.equal(state.frame.metrics.unconverged,0);
    assert.ok(state.frame.metrics.plastic_strain<=objects[i].components.plasticity.max_strain+0.00001);
    assert.ok(after.width<before.width-0.03);assert.ok(after.height>before.height+0.03);
    assert.ok(Math.abs(after.perimeter/before.perimeter-1)<0.0001);
  }
  assert.ok(barrelShape(result.review[0]).width>barrelShape(result.review[5]).width+0.15);
  await screenshot('crush-six');
  // Recompute the same recipe without dropping the favorite or changing WASM.
  await click(1350,190); await until('wasm_exports.comparison_tick()===960','same settings replay');
  await click(700,42); await seek(960); await click(885,42);
  assert.deepEqual((await saved()).review,result.review);assert.equal((await saved()).favorite,5);
  // Edit the common test while all six cards remain visible. No reload/rebuild.
  await field(265,190,'1.5');await field(400,190,'30');await field(535,190,'0.4');
  await field(670,190,'6000');await field(810,190,'4');await click(885,42);
  let edited=await saved();sameImpact(edited);
  assert.equal(edited.variants[0].impact.tank_speed,1.5);assert.equal(edited.variants[0].impact.angle_degrees,30);
  assert.equal(edited.variants[0].impact.aim[1],0.4);assert.equal(edited.variants[0].impact.tank_mass,6000);
  assert.equal(edited.duration_ticks,480);assert.deepEqual(edited.variants.map(v=>v.object),objects);
  for(const state of edited.review) {
    const geometry=new Map(state.geometry.map(g=>[g.id,g]));
    const tank=state.frame.poses.find(p=>geometry.get(p[0]).material===4);
    assert.ok(Math.abs(tank[2]-Math.PI/6)<0.00001,'actual initial tank heading');
  }
  await click(1350,190);await until('wasm_exports.comparison_tick()===480','shared angled trial');
  await click(700,42);await seek(480,480);await click(885,42);
  const angled=await saved();sameImpact(angled);
  assert.ok(angled.review.every(r=>r.frame.tick===480 && r.frame.metrics.unconverged===0));
  await screenshot('crush-shared-angle');
  await click(1350,190);await until('wasm_exports.comparison_tick()===480','angled replay');
  await click(700,42);await seek(480,480);await click(885,42);
  assert.deepEqual((await saved()).review,angled.review);
  await click(1240,124);await click(885,42);
  await click(1150,42);await click(600,220);
  assert.equal(await evaluate('wasm_exports.comparison_favorite()'),6);
  await click(885,42);sameImpact(await saved());
  assert.deepEqual((await saved()).review,angled.review);
  // Overlap presets, custom offset, and invalid/repaired angle remain shared.
  await field(400,190,'0');
  for(const offset of [0,-0.82,0.82]) {
    await click(990,190);await click(885,42);edited=await saved();sameImpact(edited);
    assert.equal(edited.variants[0].impact.aim[1],offset);
  }
  await field(400,190,'181');assert.equal(await evaluate('wasm_exports.comparison_valid()'),0);
  await field(400,190,'0');assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  // Edit ONE material in focus; the shared loading does not merge properties.
  await click(580,124);await field(1300,360,'2.5');await click(885,42);
  edited=await saved();sameImpact(edited);
  assert.deepEqual(edited.variants.map(v=>v.object.components.plasticity.max_strain),[0.4,0.8,1.2,1.6,2.2,2.5]);
  // Six-case sweeps stay at six, retaining common loading.
  await click(1220,700);await click(885,42);edited=await saved();sameImpact(edited);
  assert.equal(edited.variants.length,6);
  await screenshot('crush-material');
  assert.equal(await evaluate('window.crushTestExports === wasm_exports'),true,'all tuning used the same loaded WASM');
  await click(1230,826);await until('wasm_exports.editor_active()===1','editor');
  await key('KeyL','l',76);await screenshot('crush-library');await click(880,710);
  await until('wasm_exports.comparison_cases()===6 && wasm_exports.comparison_tick()===960','library launch');
  await click(700,42);await seek(720);
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});
  await delay(200);await screenshot('crush-hidpi');
  assert.deepEqual(errors,[]);
  console.log('PASS: six distinct deformation limits; identical impacts; runtime speed/angle/offset/mass/duration controls; actual angled tank poses; exact replay; favorite/save; separate object edits; six-case sweeps; invalid angle repair; library launch; high DPI; no page reload while tuning.');
} finally {
  if(originalDraft!==undefined)await evaluate(originalDraft===null?"localStorage.removeItem('fracture.object.v1')":`localStorage.setItem('fracture.object.v1',${JSON.stringify(originalDraft)})`).catch(()=>{});
  if(originalLibrary!==undefined)await evaluate(originalLibrary===null?"localStorage.removeItem('fracture.experiments.v1')":`localStorage.setItem('fracture.experiments.v1',${JSON.stringify(originalLibrary)})`).catch(()=>{});
  ws.close(); await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
