// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile, readFile } from 'node:fs/promises';

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

// Exercise actual canvas input, browser file upload/download, and persistence.
const draft = () => evaluate("JSON.parse(localStorage.getItem('fracture.object.v1'))");
async function typeText(value) {
  for (const ch of value) await cdp('Input.dispatchKeyEvent', { type: 'char', text: ch, key: ch });
  await delay(70);
}
async function field(x,y,value) {
  await click(x,y);
  await typeText(value);
  await key('Enter','Enter',13);
}
async function upload(path) {
  const {root}=await cdp('DOM.getDocument');
  const {nodeId}=await cdp('DOM.querySelector',{nodeId:root.nodeId,selector:'#experiment-file'});
  await cdp('DOM.setFileInputFiles',{nodeId,files:[path]});
  await delay(250);
}
const library = () => evaluate("JSON.parse(localStorage.getItem('fracture.experiments.v1'))");
let originalDraft,originalLibrary;
try {
  await mkdir('target/screenshots',{recursive:true});
  await mkdir('target/comparison-smoke',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.()>5','startup');
  originalDraft=await evaluate("localStorage.getItem('fracture.object.v1')");
  originalLibrary=await evaluate("localStorage.getItem('fracture.experiments.v1')");
  await evaluate("localStorage.removeItem('fracture.object.v1');localStorage.removeItem('fracture.experiments.v1')");
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.()>5','fresh startup');
  await key('KeyO','o',79);
  await until('wasm_exports.editor_active()===1','editor');
  // Manipulate actual authored Voronoi sites, including one undo per drag.
  await click(190,154); // Fracture tab edits stone points.
  await key('Home','Home',36); // fit
  await delay(100);
  const before=await draft();
  const min=before.outline.reduce((a,p)=>a.map((n,i)=>Math.min(n,p[i])),[Infinity,Infinity]);
  const max=before.outline.reduce((a,p)=>a.map((n,i)=>Math.max(n,p[i])),[-Infinity,-Infinity]);
  const center=min.map((n,i)=>(n+max[i])/2);
  const scale=Math.min(1008/10,610/7)*Math.min(2,Math.min((1008-180)/(max[0]-min[0]),(610-140)/(max[1]-min[1]))/Math.min(1008/10,610/7));
  const site=before.layout.sites[Math.floor(before.layout.sites.length/2)];
  const sx=908+(site[0]-center[0])*scale,sy=450+(site[1]-center[1])*scale;
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:sx,y:sy});
  await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:sx,y:sy,button:'left',clickCount:1});await delay(60);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:sx+8,y:sy+7,button:'left',buttons:1});await delay(100);
  await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:sx+8,y:sy+7,button:'left',clickCount:1});await delay(100);
  assert.notDeepEqual((await draft()).layout.sites,before.layout.sites,'drag moves grain site');
  await click(658,40);assert.deepEqual((await draft()).layout.sites,before.layout.sites,'undo grain drag');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'ShiftLeft',key:'Shift',windowsVirtualKeyCode:16,modifiers:8});
  await click(1300,650);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'ShiftLeft',key:'Shift',windowsVirtualKeyCode:16});
  assert.equal((await draft()).layout.sites.length,before.layout.sites.length+1,'add site');
  await key('Delete','Delete',46);assert.equal((await draft()).layout.sites.length,before.layout.sites.length,'delete site');
  await screenshot('editor-grain-points');
  await click(435,40);await click(500,172); // File > wall
  assert.equal((await draft()).layout.mode,'bricks');
  await click(190,154);await field(275,300,'0.8');assert.equal((await draft()).layout.size[0],0.8,'brick size');
  await screenshot('editor-brick-wall');
  await click(1320,40);
  await until('wasm_exports.comparison_active()===1 && wasm_exports.comparison_cases()===4','comparison range');
  assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await field(235,125,'Wall trial');
  await click(1240,170); // impact
  await field(1130,465,'1'); // duration global
  await screenshot('comparison-ready');
  await click(610,42);
  await until('wasm_exports.comparison_tick()===120 && wasm_exports.comparison_cursor()===120','shared playback completes');
  await click(100,480); // case C
  assert.equal(await evaluate('wasm_exports.comparison_selected()'),2);
  await click(1200,778);
  assert.equal(await evaluate('wasm_exports.comparison_favorite()'),3);
  await click(888,42);
  const first=(await library()).experiments[0];
  assert.equal(first.name,'Wall trial');assert.equal(first.favorite,2);assert.equal(first.cursor_tick,120);
  assert.equal(first.review.length,4);assert.ok(first.review.every(v=>v.frame.tick===120),'same time for all views');
  assert.ok(first.review.some(v=>v.frame.metrics.broken>0),'shell broke wall');
  assert.deepEqual(first.variants.map(v=>v.object.tension),[first.variants[1].object.tension*.5,first.variants[1].object.tension,first.variants[1].object.tension*1.5,first.variants[1].object.tension*2]);
  await screenshot('comparison-results');
  await click(610,42);await until('wasm_exports.comparison_tick()===120 && wasm_exports.comparison_cursor()===120','replay');
  await click(888,42);
  assert.deepEqual((await library()).experiments[0].review,first.review,'same recipe reproduces exact WASM frames');
  await click(508,807); // scrub mid timeline
  const tick=await evaluate('wasm_exports.comparison_cursor()');assert.ok(tick>0&&tick<120);
  await click(888,42);
  const saved=(await library()).experiments[0];assert.equal(saved.cursor_tick,tick);
  await click(800,42);assert.equal(await evaluate('wasm_exports.comparison_cursor()'),0,'rewind');
  await click(1150,42);await click(650,215); // library load
  assert.equal(await evaluate('wasm_exports.comparison_cursor()'),tick,'load saved visual checkpoint');
  await click(1300,42);await until('wasm_exports.editor_active()===1','return editor');
  assert.deepEqual((await draft()),saved.variants[2].object,'selected variant returns to editor');
  await click(1320,40);await until('wasm_exports.comparison_active()===1','return comparison');
  assert.equal(await evaluate('wasm_exports.comparison_cursor()'),tick,'unmodified round trip preserves results');
  // Create a second reproducible experiment with controlled tank impacts.
  await field(235,125,'Tank trial');
  await click(1240,170);await click(1300,220);await field(1300,280,'6');
  await click(1220,533); // copy impact to all
  await click(610,42);await until('wasm_exports.comparison_tick()===120 && wasm_exports.comparison_cursor()===120','tank run');
  await click(888,42);
  const tank=(await library()).experiments[1];assert.equal(tank.variants[0].impact.kind,'tank');
  assert.ok(tank.review.some(v=>v.frame.metrics.broken>0),'tank collision breaks object');
  await screenshot('comparison-tank');
  const dir=process.cwd()+'/target/comparison-smoke';
  await cdp('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:dir});
  await click(960,42);await delay(500);
  assert.deepEqual(JSON.parse(JSON.stringify(JSON.parse(await readFile(dir+'/Tank-trial.json','utf8')))),tank,'exported complete experiment');
  await cdp('Page.setInterceptFileChooserDialog',{enabled:true});await click(1050,42);
  await writeFile(dir+'/saved.json',JSON.stringify(saved));await upload(dir+'/saved.json');
  assert.equal(await evaluate('wasm_exports.comparison_cursor()'),tick,'import restores saved time');
  await writeFile(dir+'/bad.json',JSON.stringify({...saved,version:99}));await upload(dir+'/bad.json');
  assert.equal(await evaluate('wasm_exports.comparison_cursor()'),tick,'bad import leaves active experiment unchanged');
  await click(888,42);assert.deepEqual((await library()).experiments[0],saved,'bad import does not overwrite recipe');
  await cdp('Page.reload');await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.()>5','reload');
  await key('KeyO','o',79);await click(1320,40);await until('wasm_exports.comparison_active()===1','open after reload');
  await click(1150,42);await click(650,215);assert.equal(await evaluate('wasm_exports.comparison_cursor()'),tick,'library survives reload');
  // Material and mesh sweeps author actual reproducible values, and edits
  // discard stale recordings before showing the next experiment.
  await click(1120,173); // material
  await field(1300,238,'20');
  assert.equal(await evaluate('wasm_exports.comparison_cursor()'),0,'physical edit invalidates old playback');
  await click(1240,655); // stiffness
  await click(1210,700);
  await field(235,125,'Stiffness sweep');await click(888,42);
  const stiffness=(await library()).experiments.find(e=>e.name==='Stiffness sweep');
  assert.ok(stiffness.variants.every((v,i)=>Math.abs(v.object.young-10e9*(i+1))<5000),'stiffness sweep stores physical modulus');
  await click(1240,655);await click(1240,655); // grain size
  await click(1210,700);await field(235,125,'Mesh sweep');await click(888,42);
  const mesh=(await library()).experiments.find(e=>e.name==='Mesh sweep');
  assert.deepEqual(mesh.variants.map(v=>v.object.layout.size[1]),[0.25,0.5,0.75,1]);
  assert.ok(mesh.review[0].frame.metrics.cells>mesh.review[3].frame.metrics.cells,'grain sweep changes actual geometry');
  await field(1120,238,'-100');assert.equal(await evaluate('wasm_exports.comparison_valid()'),0);
  await click(888,42);assert.deepEqual((await library()).experiments.find(e=>e.name==='Mesh sweep'),mesh,'invalid case cannot replace saved experiment');
  await field(1120,238,'1600');assert.equal(await evaluate('wasm_exports.comparison_valid()'),1);
  await click(1310,173);await screenshot('comparison-mesh-controls');
  await cdp('Emulation.setDeviceMetricsOverride',{width:900,height:700,deviceScaleFactor:2,mobile:false});await delay(150);
  await screenshot('comparison-high-dpi');
  assert.deepEqual(errors,[],'no browser errors');
  console.log('Comparison smoke passed: grain editing, bricks, stiffness/mesh sweeps, invalid parameters, shell/tank trials, synchronized timeline, exact replay, favorite, saved checkpoint, library, export/import, invalid import, editor round trip, reload and high DPI.');
}finally {
  for (const [key,value] of [['fracture.object.v1',originalDraft],['fracture.experiments.v1',originalLibrary]]) {
    if(value!==undefined) await evaluate(value===null?`localStorage.removeItem(${JSON.stringify(key)})`:`localStorage.setItem(${JSON.stringify(key)},${JSON.stringify(value)})`).catch(()=>{});
  }
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
