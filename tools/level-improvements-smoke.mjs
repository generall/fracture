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

async function held(code,keyValue,keyCode,fn){
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code,key:keyValue,windowsVirtualKeyCode:keyCode});await delay(40);
  try {await fn();} finally {await cdp('Input.dispatchKeyEvent',{type:'keyUp',code,key:keyValue,windowsVirtualKeyCode:keyCode});await delay(60);}
}
async function ctrl(letter){await held('ControlLeft','Control',17,()=>key('Key'+letter,letter.toLowerCase(),letter.charCodeAt(0)));}
const selected=()=>evaluate('wasm_exports.level_editor_selection_count()');
function dimensions(item){const xs=item.asset.outline.map(p=>p[0]),ys=item.asset.outline.map(p=>p[1]);return [Math.max(...xs)-Math.min(...xs),Math.max(...ys)-Math.min(...ys)];}
let original;
try {
  await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5','startup');
  original=await evaluate('({level:localStorage.getItem("fracture.level.v1"),objects:localStorage.getItem("fracture.objects.v1")})');
  await evaluate('localStorage.removeItem("fracture.level.v1");localStorage.removeItem("fracture.objects.v1")');await cdp('Page.reload');
  await until('wasm_exports?.lab_ticks?.()>5','clean startup');await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','map editor');
  // Circular authoring, diameter handles and large dimensions clamp at the map.
  await click(120,383);await mapClick(-8,-2);await click(1260,180);
  await field(1200,310,'5');let current=await map();assert.deepEqual(dimensions(current.items[4]),[5,5]);
  await field(1200,310,'300');current=await map();let d=dimensions(current.items[4]);assert.ok(d[0]>29&&d[0]<30);assert.equal(d[0],d[1]);
  await field(1200,310,'2');current=await map();const barrel=structuredClone(current.items[4]);
  // Moving outside the canvas keeps dragging, and sticks at the arena border.
  await mapDrag(barrel.position,[-30,-20]);current=await map();assert.ok(current.items[4].position[0]<-22);assert.ok(current.items[4].position[1]<-13);
  assert.deepEqual(dimensions(current.items[4]),[2,2]);
  const base={version:1,name:'Editor interaction checks',size:[48,30],items:[]};
  const item=(position)=>({...structuredClone(barrel),position,angle:0});
  await importMap({...base,items:[item([-8,-2]),item([0,-2]),item([8,4])]});await click(120,150);
  await mapClick(-8,-2);await held('ShiftLeft','Shift',16,()=>mapClick(0,-2));assert.equal(await selected(),2);
  await mapDrag([-8,-2],[-6,0]);current=await map();assert.ok(Math.abs(current.items[0].position[0]+6)<0.03);assert.ok(Math.abs(current.items[1].position[0]-2)<0.03);
  await field(1200,370,'0');current=await map();assert.ok(Math.abs(current.items[0].position[0]+4)<0.04);assert.ok(Math.abs(current.items[1].position[0]-4)<0.04);
  await ctrl('C');await ctrl('V');assert.equal((await map()).items.length,5);assert.equal(await selected(),2);
  await ctrl('Z');assert.equal((await map()).items.length,3);await ctrl('Y');assert.equal((await map()).items.length,5);
  await key('Delete','Delete',46); // history cleared selection, deliberately select all first
  await ctrl('A');assert.equal(await selected(),5);await key('Delete','Delete',46);assert.equal((await map()).items.length,0);await ctrl('Z');
  // Empty-space box selection, including additive selection, stays off text fields.
  await importMap({...base,items:[item([-8,-2]),item([0,-2]),item([8,4])]});
  await mapDrag([-11,-5],[3,1]);assert.equal(await selected(),2);
  await screenshot('level-multi-select');
  await ctrl('X');assert.equal((await map()).items.length,1);await ctrl('V');assert.equal((await map()).items.length,3);
  // Connected rocks become a real union. Undo restores both source objects.
  await importMap(base);await click(50,201);await click(120,523);await mapClick(-1,0);await mapClick(1,0);
  assert.equal((await map()).items.length,1,'automatic rock union');assert.ok(dimensions((await map()).items[0])[0]>4);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1100,y:80});await delay(60);await screenshot('level-merged-rock');await ctrl('Z');assert.equal((await map()).items.length,1,'one placement plus merge is one undo');
  // Disable automatic joining to explicitly select and join overlapping walls.
  await importMap(base);await click(120,745);await click(120,250);await mapDrag([-7.8,-2.8],[0.8,-2.2]);await mapDrag([-0.8,-2.8],[7.8,-2.2]);
  assert.equal((await map()).items.length,2);await click(120,150);await ctrl('A');await ctrl('J');assert.equal((await map()).items.length,1);
  await ctrl('Z');assert.equal((await map()).items.length,2);await ctrl('Y');assert.equal((await map()).items.length,1);
  // A corner drag also clamps size without cancelling it.
  await mapClick(0,-2.5);const wall=(await map()).items[0],wallSize=dimensions(wall);await mapDrag([wall.position[0]+wallSize[0]/2,wall.position[1]+wallSize[1]/2],[40,0]);
  assert.ok(dimensions((await map()).items[0])[0]>40);
  // Compatible wall ends snap into contact when moved close, then join.
  await importMap(base);await click(120,745);await click(120,250);await mapDrag([-7.8,-5.8],[-4.2,-5.2]);await mapDrag([0.2,-5.8],[3.8,-5.2]);await click(120,150);
  await mapDrag([2,-5.5],[-2.15,-5.5]);assert.equal((await map()).items.length,1,'wall edge magnet and automatic join');assert.ok(Math.abs(dimensions((await map()).items[0])[0]-8)<0.12);
  // Large bridge sizes use a cheap dedicated mesh, and booth footprint clamps too.
  await importMap({...base,size:[200,120]});await key('Home','Home',36);await delay(100);
  await click(144,201);await click(120,458);await mapDrag([-60,-10],[60,10]);
  assert.equal((await map()).items.length,1);let bridge=(await map()).items[0];assert.equal(bridge.kind,'bridge');assert.ok(bridge.max[0]-bridge.min[0]>119);
  await click(1260,180);await field(1200,310,'300');bridge=(await map()).items[0];assert.ok(bridge.max[0]-bridge.min[0]>189);
  await screenshot('level-large-bridge');await ctrl('C');await importMap(base);await ctrl('V');assert.equal((await map()).items.length,1);bridge=(await map()).items[0];assert.ok(bridge.max[0]-bridge.min[0]<48,'clipboard group scales to smaller map');
  // Cutting and pasting a unique team marker restores it; copying cannot duplicate it.
  await importMap({...base,items:[{kind:'flag',team:'blue',position:[0,0]}]});await click(120,150);await mapClick(0,0);await ctrl('X');assert.equal((await map()).items.length,0);await ctrl('V');assert.equal((await map()).items.length,1);await ctrl('C');await ctrl('V');assert.equal((await map()).items.length,1);
  // Explicit unbreakable wall tool, and merged terrain presentation.
  await importMap(base);await key('Home','Home',36);await delay(100);
  await click(50,201);await click(120,299);await mapDrag([-11.8,6.2],[11.8,6.8]);assert.equal((await map()).items[0].kind,'solid');
  await click(144,201);await click(120,349);await mapDrag([-15,-10],[0,0]);await mapDrag([-6,-8],[12,3]);
  await click(120,397);await mapDrag([-2,-7],[3,6]);await mapDrag([-2,2],[10,7]);
  await click(120,150);await screenshot('level-merged-terrain');
  // Every drawing tool fills complete cells, regardless of the movement snap toggle.
  const gridBounds=item=>item.min ? [item.min,item.max] : [
    item.position.map((n,i)=>n+Math.min(...(item.outline||item.asset.outline).map(p=>p[i]))),
    item.position.map((n,i)=>n+Math.max(...(item.outline||item.asset.outline).map(p=>p[i])))
  ];
  for (const [category,tool,kind] of [[50,250,'object'],[50,299,'solid'],[225,254,'solid'],[144,254,'terrain'],[144,301,'terrain'],[144,349,'terrain'],[144,397,'terrain']]) {
    await importMap(base);await click(category,201);await click(120,tool);
    await mapClick(-0.05,-1.95);
    assert.equal((await map()).items[0].kind,kind);
    assert.deepEqual(gridBounds((await map()).items[0]),[[-1,-2],[0,-1]],'click fills the cell under the pointer');
    await ctrl('Z');assert.equal((await map()).items.length,0);
    await mapDrag([-2.85,0.15],[3.95,2.95]);
    const forward=await map();assert.deepEqual(gridBounds(forward.items[0]),[[-3,0],[4,3]],'drag fills endpoint cells');
    await ctrl('Z');await click(120,790); // movement snapping must not round the drawing cursor into another cell
    await mapDrag([3.95,2.95],[-2.85,0.15]);assert.deepEqual(await map(),forward,'reverse drag and movement snap preserve fill');
    await click(120,790);await ctrl('Z');await ctrl('Y');assert.deepEqual(await map(),forward,'grid fills support undo/redo');
  }
  await importMap(base);await click(144,201);await click(120,458);await mapClick(22.8,13.8);
  let gridBridge=(await map()).items[0];assert.equal(gridBridge.kind,'bridge');assert.deepEqual([gridBridge.min,gridBridge.max],[[20,10],[23,12]],'minimum bridge stays on grid with booth clearance');
  await importMap(base);await click(50,201);await click(120,299);await mapDrag([-8.8,-4.8],[4.8,-2.2]);
  const [previewX,previewY]=await at(7.4,3.4);
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:previewX,y:previewY});await delay(80);
  await screenshot('level-grid-drawing');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});await delay(200);await screenshot('level-improvements-high-dpi');
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({result:'PASS',checks:['circular barrels','diameter clamping','border drag','shift selection','group move','copy paste','cut','select all','marquee','group undo redo','automatic rock union','explicit wall union','corner clamp','large bridge','unbreakable wall tool','terrain overlaps','grid cell click','grid rectangle drag','reverse grid drag','movement snap independence','grid undo redo','bridge grid clearance','high DPI']},null,2));
} catch(error){await screenshot('level-improvements-failure');console.log('DRAFT',JSON.stringify(await map()));throw error;} finally {
  if(original) await evaluate(`(()=>{for(const [key,value]of Object.entries({'fracture.level.v1':${JSON.stringify(original.level)},'fracture.objects.v1':${JSON.stringify(original.objects)}})){if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value);}})()`).catch(()=>{});
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});ws.close();
}
