// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.SANDBOX_URL || 'http://127.0.0.1:8000/sandbox.html');
sandboxUrl.searchParams.delete('mode');
const url = sandboxUrl.href;
const demo = new URL(url); demo.searchParams.set('experiment','barrel-shot');
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

const keys=['fracture.object.v1','fracture.experiments.v1'];
let originals;
const saved = async () => (await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))")).experiments.find(e => e.name === 'Barrels / shot response / 6 cases');
async function seek(tick,duration=360) {await click(28+1384*tick/duration,817);}
async function field(x,y,value) {
  await click(x,y);
  for(const ch of value)await cdp('Input.dispatchKeyEvent',{type:'char',text:ch,key:ch});
  await delay(70);await key('Enter','Enter',13);
}
function centerX(state) {
  const geometry=new Map(state.geometry.map(g=>[g.id,g]));
  const poses=state.frame.poses.filter(p=>geometry.get(p[0]).material===2);
  return poses.reduce((sum,p)=>sum+p[1][0],0)/poses.length;
}
function sameImpact(e) {
  assert.equal(e.shared_impact,true);
  assert.ok(e.variants.every(v=>JSON.stringify(v.impact)===JSON.stringify(e.variants[0].impact)));
}
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_cases?.()===6','six shell cases');
  originals=await evaluate(`JSON.stringify(${JSON.stringify(keys)}.map(k=>localStorage.getItem(k)))`);
  await evaluate("localStorage.removeItem('fracture.experiments.v1')");
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_tick?.()===360','recorded shell trials');
  await evaluate('window.shotTestExports=wasm_exports');
  await click(700,42);await seek(0);await click(885,42);
  const initial=await saved();sameImpact(initial);
  assert.equal(initial.favorite,5);
  assert.deepEqual(initial.variants.map(v=>v.object.friction),[.02,.09,.4,.02,.09,.4]);
  assert.deepEqual(initial.variants.map(v=>v.object.restitution),[.04,.04,.04,.7,.7,.7]);
  assert.ok(initial.variants.every(v=>v.impact.kind==='shell' && v.impact.backstop));
  for(const v of initial.variants)assert.deepEqual(v.object.components.plasticity,{yield_stress:500,flow_rate:2,hardening:0,max_strain:3});
  await screenshot('barrel-shot-start');
  await seek(360);await click(710,550);await click(1240,124);await click(885,42);
  const result=await saved();assert.equal(result.favorite,4);
  assert.ok(result.review.every(r=>r.frame.shells.length===0 && r.frame.metrics.broken===0 && r.frame.metrics.unconverged===0));
  const xs=result.review.map(centerX);
  assert.ok(xs[3]<xs[0]-1 && xs[3]<xs[4]-.5 && xs[4]<xs[5]-.1);
  await screenshot('barrel-shot-six');
  await click(1350,190);await until('wasm_exports.comparison_tick()===360','replay');
  await click(700,42);await seek(360);await click(885,42);
  assert.deepEqual((await saved()).review,result.review);
  assert.equal((await saved()).favorite,4);
  // Shared shell controls leave all six different materials intact.
  await field(265,190,'160');await field(535,190,'0.25');
  await field(670,190,'4');await field(810,190,'2');
  await click(1350,190);await until('wasm_exports.comparison_tick()===240','edited shot');
  await click(700,42);await seek(240,240);await click(885,42);
  let e=await saved();sameImpact(e);
  assert.equal(e.variants[0].impact.shell_speed,160);
  assert.equal(e.variants[0].impact.shell_mass,4);
  assert.equal(e.variants[0].impact.aim[1],.25);
  assert.deepEqual(e.variants.map(v=>v.object),initial.variants.map(v=>v.object));
  await click(580,124);await field(1100,483,'1.1');
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),0);
  await field(1100,483,'0.35');await field(1100,422,'0.18');
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await click(885,42);e=await saved();sameImpact(e);
  assert.deepEqual(e.variants.map(v=>v.object.restitution),[.04,.04,.04,.7,.35,.7]);
  assert.deepEqual(e.variants.map(v=>v.object.friction),[.02,.09,.4,.02,.18,.4]);
  assert.equal(await evaluate('window.shotTestExports===wasm_exports'),true);
  await screenshot('barrel-shot-material');
  await click(1230,826);await until('wasm_exports.editor_active()===1','editor');
  await click(307,154);await field(270,418,'0.55');await field(270,382,'0.25');
  const draft=await evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
  assert.equal(draft.restitution,.55);assert.equal(draft.friction,.25);
  await screenshot('barrel-shot-editor');
  await key('KeyL','l',76);await click(620,710);
  await until('wasm_exports.comparison_cases()===6 && wasm_exports.comparison_tick()===360','library shot shortcut');
  await click(700,42);await seek(120);
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});
  await delay(200);await screenshot('barrel-shot-hidpi');
  // The separately named no-backstop reference remembers the other choice.
  const freeUrl=new URL(url);freeUrl.searchParams.set('experiment','barrel-shot-free');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.navigate',{url:freeUrl.href});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_favorite?.()===2 && wasm_exports?.comparison_tick?.()===360','free reference startup');
  await click(700,42);await seek(120);await click(885,42);
  const free=(await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))")).experiments.find(e=>e.name==='Barrels / free shot / 6 cases');
  assert.equal(free.favorite,1);sameImpact(free);
  assert.ok(free.variants.every(v=>!v.impact.backstop));
  assert.deepEqual(free.variants.map(v=>v.object),initial.variants.map(v=>v.object));
  for(const state of free.review) {
    const walls=state.geometry.filter(g=>g.material===5).map(g=>g.id);
    assert.equal(walls.length,4);
    assert.equal(state.frame.poses.filter(p=>walls.includes(p[0])).length,4);
    assert.ok(state.frame.metrics.largest_area<1.4,'walls must not count as remaining barrel area');
  }
  await screenshot('barrel-shot-free');
  // New material: separate from the close wall, preserve the old free shot.
  const releaseUrl=new URL(url);releaseUrl.searchParams.set('experiment','barrel-release');
  await cdp('Page.navigate',{url:releaseUrl.href});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.comparison_favorite?.()===2 && wasm_exports?.comparison_selected?.()===1 && wasm_exports?.comparison_tick?.()===360','release trial');
  await evaluate('window.releaseTestExports=wasm_exports');
  await click(700,42);await seek(360);await click(885,42);
  const releaseSaved=async()=> (await evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))")).experiments.find(e=>e.name==='Barrels / wall release / 6 cases');
  let release=await releaseSaved();sameImpact(release);
  assert.equal(release.favorite,1);
  assert.deepEqual(release.variants.map(v=>v.object.soft_bounce.restitution),[.6,.62,.64,.66,.68,.7]);
  assert.ok(release.variants.every(v=>v.object.soft_bounce.speed===16 && v.object.friction===.09 && v.object.restitution===.04));
  const right=state=>Math.max(...state.frame.poses.flatMap(p=> {
    const g=state.geometry.find(g=>g.id===p[0]);if(g.material!==2)return [];
    return g.vertices.slice(0,g.count).map(([x,y])=>p[1][0]+Math.cos(p[2])*x-Math.sin(p[2])*y);
  }));
  const gap=3.8-right(release.review[1]);assert.ok(gap>.3 && gap<1);
  await screenshot('barrel-release-six');
  await click(1190,190);await click(1350,190);
  await until('wasm_exports.comparison_tick()===360','release without backstop');
  await click(700,42);await seek(120);await click(885,42);release=await releaseSaved();
  const before=free.review[1],after=release.review[1];
  assert.equal(before.frame.tick,after.frame.tick);
  assert.equal(before.frame.poses.length,after.frame.poses.length);
  for(let i=0;i<before.frame.poses.length;i++) {
    const p=before.frame.poses[i],q=after.frame.poses[i];assert.equal(p[0],q[0]);
    assert.ok(Math.hypot(p[1][0]-q[1][0],p[1][1]-q[1][1])<.001);
    assert.ok(Math.abs(p[2]-q[2])<.001);
  }
  await click(580,124);await click(1160,529);
  await field(1100,300,'0.65');await field(1300,300,'0');
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),0);
  await field(1300,300,'16');assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await click(885,42);release=await releaseSaved();
  assert.deepEqual(release.variants.map(v=>v.object.soft_bounce.restitution),[.6,.65,.64,.66,.68,.7]);
  assert.equal(await evaluate('window.releaseTestExports===wasm_exports'),true);
  await screenshot('barrel-release-material');
  await click(1230,826);await until('wasm_exports.editor_active()===1','release editor');
  await click(307,154);await click(300,740);await field(270,418,'0.66');
  let updated=await evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
  assert.deepEqual(updated.soft_bounce,{restitution:.66,speed:16});
  await click(658,42);updated=await evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
  assert.equal(updated.soft_bounce.restitution,.65);
  await screenshot('barrel-release-editor');
  assert.deepEqual(errors,[]);
  console.log('PASS: six friction/bounce variants; shared shot; measurable rebound and stopping differences; exact replay; favorite/save; runtime shell edits; separate material edits and validation; editor persistence; library shortcut; high DPI; separate backstop/free favorites; recorded outer walls; speed-dependent release; retained free movement; editable bounce curve and undo.');
} finally {
  if(originals!==undefined)await evaluate(`(${JSON.stringify(JSON.parse(originals))}).forEach((v,i)=>v===null?localStorage.removeItem(${JSON.stringify(keys)}[i]):localStorage.setItem(${JSON.stringify(keys)}[i],v))`).catch(()=>{});
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
