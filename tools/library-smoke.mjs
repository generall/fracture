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
// Components must survive real editor input, library persistence and reload.
const keys=['fracture.object.v1','fracture.objects.v1'];
let originals;
const openLibrary=async()=>{await key('KeyL','l',76);await until('wasm_exports.editor_library_open()===1','library open');};
const starter=async i=>{await openLibrary();await click(285,198);await click(450+(i%2)*524,302+Math.floor(i/2)*200);await until('wasm_exports.editor_library_open()===0','starter loaded');};
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','startup');
  originals=await evaluate(`JSON.stringify(${JSON.stringify(keys)}.map(k=>localStorage.getItem(k)))`);
  await evaluate(`${JSON.stringify(keys)}.forEach(k=>localStorage.removeItem(k))`);
  await cdp('Page.reload');
  await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','clean startup');
  await key('KeyO','o',79);
  await fileAction(410);
  await until('wasm_exports.editor_library_open()===1','File library entry');
  await screenshot('object-library');
  // Modal absorbs Enter and clicks, keeping the simulation/editor unchanged.
  await key('Enter','Enter',13);
  assert.equal(await evaluate('wasm_exports.editor_active()'),1);
  await click(450,302);
  let a=await draft();assert.equal(a.name,'barrel');assert.equal(a.outline.length,16);
  assert.deepEqual(a.components.plasticity,{yield_stress:500,flow_rate:2,hardening:0,max_strain:3});
  assert.deepEqual(a.color,[105,224,174]);assert.ok(a.components.crush);
  await field(270,218,'my green barrel');
  await field(270,670,'#55EEDD');
  await tab(2);await click(185,740);
  await field(270,276,'55');await field(270,418,'3.5');
  await click(185,516);await field(270,560,'800');await field(270,602,'2.5');
  a=await draft();assert.equal(a.components.projectile_resistance,55);assert.equal(a.components.crush.force,3500);
  assert.deepEqual(a.components.explosive,{energy:800000,radius:2.5});assert.deepEqual(a.color,[85,238,221]);
  await screenshot('object-behavior');
  await openLibrary();await click(300,710);
  await until('wasm_exports.editor_library_saved()===1','save custom copy');
  await click(500,198);await screenshot('object-library-saved');
  await key('Escape','Escape',27);
  await field(270,276,'1001'); // Invalid multiplier must not replace a saved copy.
  await openLibrary();await click(300,710);
  assert.equal(await evaluate("JSON.parse(localStorage.getItem('fracture.objects.v1')).objects[0].components.projectile_resistance"),55);
  await key('Escape','Escape',27);
  // Browser reload must restore the last valid draft and independent library.
  await cdp('Page.reload');await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','reload');
  await key('KeyO','o',79);await openLibrary();await click(500,198);
  assert.equal(await evaluate('wasm_exports.editor_library_saved()'),1);
  await click(700,268);assert.deepEqual(await draft(),a);
  const colors=[];
  for(let i=0;i<4;i++) {
    await starter(i);const object=await draft();colors.push(object.color.join(','));
    assert.ok(await evaluate('wasm_exports.editor_cells()')>0);
    if(i===0 || i===1) {
      await click(1160,40);await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});
      await delay(100);await key('KeyF','f',70);
      await until(`wasm_exports.lab_explosions()>=${i===0?1:2}`,'shell and barrel event');
      const s=await stats();assert.equal(s.explosions,i===0?1:2);assert.equal(s.unconverged,0);
      if(i===0)assert.equal(s.broken,0);else assert.equal(s.cells,0);
      await key('KeyO','o',79);
    }
    if(i===2) {
      assert.equal(object.pinned_edge,2);
      assert.ok(!object.additional_pinned_edges);
      await tab(0);
      await click(185,462); // Add a second support without replacing the first.
      assert.deepEqual((await draft()).additional_pinned_edges,[0]);
      assert.equal((await draft()).pinned_edge,2);
      await click(658,42);
      assert.ok(!(await draft()).additional_pinned_edges);
      assert.equal((await draft()).pinned_edge,2);
      await screenshot('library-wall-editor');
      await click(1160,40);await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:846,y:442});
      await delay(100);await key('KeyF','f',70);
      await until('wasm_exports.lab_explosions()===1 && wasm_exports.lab_chunks()>2','wall breaks on first shell');
      assert.equal((await stats()).unconverged,0);
      await screenshot('library-wall-shot');
      await key('KeyO','o',79);
    }
    if(i===3) {
      assert.equal(object.ground_anchor,true);
      await tab(0);await click(185,510);
      assert.ok(!(await draft()).ground_anchor);
      await click(658,42);assert.equal((await draft()).ground_anchor,true);
      await cdp('Page.reload');await until('typeof wasm_exports !== "undefined" && wasm_exports?.lab_ticks?.() > 5','anchored draft reload');
      await key('KeyO','o',79);assert.equal((await draft()).ground_anchor,true);
    }
  }
  assert.equal(new Set(colors).size,4);
  await screenshot('library-rock-editor');
  await openLibrary();await click(1130,710);
  await until('wasm_exports.editor_active()===0 && wasm_exports.lab_ticks()>5','yard');
  assert.ok((await stats()).cells>60);
  await screenshot('object-yard');
  await key('Key1','1',49);await key('Key4','4',52);await delay(100);
  assert.ok((await stats()).cells>60);
  assert.deepEqual(errors,[]);
  console.log('Object library browser checks passed: four starters, modal input, editable components/color, save/load/reload, invalid save preservation, shell behavior, yard.');
} finally {
  if(originals) {
    const values=JSON.parse(originals);
    await evaluate(`${JSON.stringify(keys)}.forEach((k,i)=>{const v=${JSON.stringify(values)}[i];if(v===null)localStorage.removeItem(k);else localStorage.setItem(k,v)})`).catch(()=>{});
  }
  ws.close();await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
