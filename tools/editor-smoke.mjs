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
  const {nodeId}=await cdp('DOM.querySelector',{nodeId:root.nodeId,selector:'input[type=file]'});
  await cdp('DOM.setFileInputFiles',{nodeId,files:[path]});
  await delay(250);
}
async function tab(index) {await click(75+116*index,154);}
async function preset(index) {await click(435,40);await click(500,132+40*index);}
async function fileAction(y) {await click(435,40);await click(500,y);}
let originalDraft;
try {
  await mkdir('target/screenshots',{recursive:true});
  await mkdir('target/editor-smoke',{recursive:true});
  await cdp('Runtime.enable');
  await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','startup');
  originalDraft=await evaluate("localStorage.getItem('fracture.object.v1')");
  await evaluate("localStorage.removeItem('fracture.object.v1')");
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','fresh startup');
  await click(1150,40);
  await until('wasm_exports.editor_active()===1','open editor button');
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),7);
  assert.ok(await evaluate('wasm_exports.editor_cells()')>0);
  assert.equal((await draft()).layout.mode,'stones');
  const frozen=await stats();
  await delay(250);
  assert.equal((await stats()).ticks,frozen.ticks,'editor pauses the firing range');
  await screenshot('editor-rock');
  assert.equal(await evaluate('wasm_exports.editor_tab()'),0,'starts in Shape');
  const untouched=await draft();
  await click(435,40);assert.equal(await evaluate('wasm_exports.editor_menu()'),1,'File dropdown opens');
  await screenshot('editor-file-menu');
  await key('Escape','Escape',27);assert.equal(await evaluate('wasm_exports.editor_menu()'),0,'Escape dismisses dropdown');
  assert.equal(await evaluate('wasm_exports.editor_active()'),1,'Escape in dropdown keeps editor open');
  await click(545,40);assert.equal(await evaluate('wasm_exports.editor_menu()'),2,'View dropdown opens');
  await screenshot('editor-view-menu');
  await click(908,450);assert.equal(await evaluate('wasm_exports.editor_menu()'),0,'outside click dismisses dropdown');
  assert.deepEqual(await draft(),untouched,'dropdown dismissal does not edit canvas');
  await tab(2);await key('Delete','Delete',46);
  assert.deepEqual(await draft(),untouched,'Delete in Material does not remove a vertex');
  await screenshot('editor-material-basic');
  await click(180,458);await screenshot('editor-material-advanced');
  await tab(1);await screenshot('editor-fracture-tools');

  await preset(2); // Plate preset.
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),4);
  await tab(1);await click(305,248); // Legacy mesh for the existing regression fixture.
  assert.equal(await evaluate('wasm_exports.editor_cells()'),32);
  await tab(2);await field(276,310,'3200');
  assert.equal((await draft()).density,3200,'typed density persisted');
  await field(276,352,'0.5');
  assert.equal((await draft()).thickness,0.5,'typed thickness persisted');
  assert.equal(await evaluate('wasm_exports.editor_active()'),1,'Enter in a field does not launch testing');
  await tab(1);await click(175,712);await click(329,758);
  assert.equal(await evaluate('wasm_exports.editor_cells()'),128,'detail changes mesh');
  await tab(0);await click(100,405);
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),5,'add inserts on selected edge');
  await click(658,40);
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),4,'undo shape edit');
  await click(750,40);
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),5,'redo shape edit');
  await click(270,405);
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),4,'delete selected vertex');
  await field(276,310,'2.0');
  assert.equal((await draft()).outline[1][0],2.0,'numeric vertex editing');
  // Undo that edit so the rectangle has known coordinates for dragging.
  await click(658,40);
  await key('Home','Home',36); // Fit: plate is centered at (908,450), scale 174.29.
  await delay(100);
  const beforeDrag=await draft();
  // Current auto-fit is capped at 2x: base=610/7 -> scale=174.29.
  const sx=908-1.7*(610/7*2), sy=450-1.2*(610/7*2);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:sx,y:sy});
  await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:sx,y:sy,button:'left',clickCount:1});
  await delay(60);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:sx-32,y:sy+32,button:'left',buttons:1});
  await delay(100);
  await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:sx-32,y:sy+32,button:'left',clickCount:1});
  await delay(100);
  assert.notDeepEqual((await draft()).outline,beforeDrag.outline,'drag changes polygon');
  await click(658,40);
  assert.deepEqual((await draft()).outline,beforeDrag.outline,'one undo reverses whole drag');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'ShiftLeft',key:'Shift',windowsVirtualKeyCode:16,modifiers:8});
  await click(908,sy);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'ShiftLeft',key:'Shift',windowsVirtualKeyCode:16});
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),5,'Shift-click inserts a vertex on an edge');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'ControlLeft',key:'Control',windowsVirtualKeyCode:17,modifiers:2});
  await key('KeyZ','z',90);
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'ControlLeft',key:'Control',windowsVirtualKeyCode:17});
  assert.equal(await evaluate('wasm_exports.editor_vertices()'),4,'keyboard undo reverses edge insertion');
  // Select the first vertex again after undo preserves the previous index.
  await click(sx,sy);
  await click(174,463);
  assert.equal((await draft()).pinned_edge,0,'pin selected edge');
  await screenshot('editor-plate');
  const edited=await draft();
  await key('Enter','Enter',13);
  await until('wasm_exports.editor_active()===0 && wasm_exports.lab_cells()===128 && wasm_exports.lab_chunks()===1 && wasm_exports.lab_bodies()===1','test creates exact mesh');
  await delay(500);
  assert.equal((await stats()).broken,0,'custom pinned object stable before shot');
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});await delay(60);
  await key('KeyF','f',70);
  await until('wasm_exports.lab_explosions()===1','test object hit');
  await screenshot('editor-test');
  await key('KeyR','r',82);
  assert.equal((await stats()).cells,128,'reset rebuilds custom object');
  assert.equal((await stats()).shots,0);
  await key('KeyO','o',79);
  await until('wasm_exports.editor_active()===1','return from testing');
  assert.deepEqual(await draft(),edited,'test preserves authored draft');
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','reload');
  await key('KeyO','o',79);
  await until('wasm_exports.editor_active()===1','reopen saved draft');
  assert.deepEqual(await draft(),edited,'reload restores draft');
  const imported={...edited,name:'Editor roundtrip',density:2700,refinement:1,pinned_edge:null};
  const dir=process.cwd()+'/target/editor-smoke';
  await writeFile(dir+'/import.json',JSON.stringify(imported));
  await cdp('Page.setInterceptFileChooserDialog',{enabled:true});
  await fileAction(320);
  await upload(dir+'/import.json');
  assert.deepEqual(await draft(),imported,'file import replaces draft');
  const invalid={...imported,outline:[[0,0],[1,1],[1,0],[0,1]]};
  await writeFile(dir+'/invalid.json',JSON.stringify(invalid));
  await upload(dir+'/invalid.json');
  assert.deepEqual(await draft(),imported,'invalid file leaves draft intact');
  await screenshot('editor-invalid-import');
  await cdp('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:dir});
  await fileAction(364);
  await delay(400);
  assert.deepEqual(JSON.parse(await readFile(dir+'/Editor-roundtrip.json','utf8')),imported,'download round-trip');
  const triangle={...imported,name:'Adaptive triangle',outline:[[-0.8,-0.6],[0.8,-0.6],[0,0.6]],surface:'rock',density:2600,thickness:0.25,tension:260000,compression:8000000,shear:380000,cohesion:1,refinement:0};
  await writeFile(dir+'/triangle.json',JSON.stringify(triangle));
  await upload(dir+'/triangle.json');
  await key('Enter','Enter',13);
  await until('wasm_exports.editor_active()===0 && wasm_exports.lab_cells()===1 && wasm_exports.lab_chunks()===1','isolated triangle starts as one cell and chunk');
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});await delay(60);
  await key('KeyF','f',70);
  await until('wasm_exports.lab_refinements()>0 && wasm_exports.lab_broken()>0','shell refines and fractures isolated triangle');
  const adaptive=await stats();
  assert.ok(adaptive.cells>1 && adaptive.chunks>1,'new geometry produces physical fragments');
  assert.equal(adaptive.unconverged,0,'adaptive stress solve converges');
  await screenshot('editor-adaptive-triangle');
  await key('KeyR','r',82);
  assert.equal((await stats()).cells,1,'reset discards runtime refinement');
  await key('KeyO','o',79);
  await until('wasm_exports.editor_active()===1','return after adaptive test');
  assert.deepEqual(await draft(),triangle,'runtime fracture never changes the authored draft');
  await upload(dir+'/import.json');
  // Invalid property must disable testing without replacing the saved draft.
  await tab(2);await field(275,310,'-10');
  assert.equal(await evaluate('wasm_exports.editor_cells()'),0);
  await click(1160,40);
  assert.equal(await evaluate('wasm_exports.editor_active()'),1,'invalid draft cannot be tested');
  assert.deepEqual(await draft(),imported,'invalid edit cannot replace saved draft');
  await click(658,40);
  assert.equal(await evaluate('wasm_exports.editor_cells()'),8);
  await cdp('Emulation.setDeviceMetricsOverride',{width:900,height:700,deviceScaleFactor:2,mobile:false});
  await delay(200);
  await screenshot('editor-high-dpi');
  assert.deepEqual(errors,[],'no browser errors');
  console.log('Editor smoke passed: task tabs, dropdown dismissal, contextual editing, shape edits, undo/redo, properties, mesh, pinning, testing, adaptive triangle fracture, reset, persistence, file round-trip, invalid input, high DPI.');
} finally {
  if (originalDraft !== undefined) await evaluate(originalDraft===null ? "localStorage.removeItem('fracture.object.v1')" : `localStorage.setItem('fracture.object.v1',${JSON.stringify(originalDraft)})`).catch(()=>{});
  ws.close();
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
