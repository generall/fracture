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
let original;
try {
  await mkdir('target/screenshots',{recursive:true});await cdp('Runtime.enable');await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.lab_ticks?.()>5 && document.getElementById("status").hidden','startup');
  original=await evaluate('({level:localStorage.getItem("fracture.level.v1"),objects:localStorage.getItem("fracture.objects.v1")})');
  await evaluate('localStorage.removeItem("fracture.level.v1");localStorage.removeItem("fracture.objects.v1")');await cdp('Page.reload');
  await until('wasm_exports?.lab_ticks?.()>5','clean startup');await key('KeyM','m',77);
  await until('wasm_exports.level_editor_active()===1','map editor');
  assert.equal(await evaluate('wasm_exports.level_editor_items()'),4);await screenshot('level-empty');
  // Draw a 20 metre wall in a single gesture, then reshape the instance.
  await click(120,250);await mapDrag([-11.8,-5.8],[7.8,-5.2]);
  assert.equal((await map()).items.length,5);
  const originalWall=(await map()).items[4];assert.equal(originalWall.kind,'object');assert.ok(originalWall.asset.outline[1][0]-originalWall.asset.outline[0][0]>19.8);
  await click(1260,180); // edit placed instance
  await field(1200,310,'24');let current=await map();assert.ok(Math.abs(current.items[4].asset.outline[1][0]-12)<0.05);
  await field(1200,380,'15');assert.ok(Math.abs((await map()).items[4].angle-Math.PI/12)<1e-5);
  const valid=await map();await field(1200,310,'300');const clamped=await map();assert.notDeepEqual(clamped,valid,'oversized dimensions clamp instead of cancelling');assert.ok(clamped.items[4].asset.outline[1][0]>12);await click(580,40);assert.deepEqual(await map(),valid,'undo clamped resize');
  await click(580,40);assert.ok(Math.abs((await map()).items[4].angle)<1e-6,'undo rotation');await click(680,40);assert.deepEqual(await map(),valid,'redo rotation');
  await mapClick(...(await map()).items[4].position);await click(1260,440); // vertex handles
  current=await map();const wall=current.items[4],p=wall.asset.outline[0],a=wall.angle;
  const point=[wall.position[0]+Math.cos(a)*p[0]-Math.sin(a)*p[1],wall.position[1]+Math.sin(a)*p[0]+Math.cos(a)*p[1]];
  await mapDrag(point,[point[0],point[1]-0.35]);assert.notDeepEqual((await map()).items[4].asset.outline,wall.asset.outline);
  await screenshot('level-wall-vertices');
  // Terrain and solids have rectangular placement; markers replace their team slot.
  await click(144,201);await click(110,301);await mapDrag([-22,-1.5],[22,1.5]); // road
  await click(110,397);await mapDrag([-2.3,-14],[2.3,14]); // water
  await click(110,458);await mapDrag([-3.4,-1.5],[3.4,1.5]); // bridge
  assert.equal((await map()).items.filter(i=>i.kind==='bridge').length,1);
  await click(225,201);await click(110,254);await mapDrag([10,5],[14,7]);
  assert.equal((await map()).items.filter(i=>i.kind==='solid').length,1);
  await click(110,321);await mapClick(-15,0);assert.equal((await map()).items.length,9);
  await click(110,478);await mapClick(6,0);assert.equal((await map()).items.filter(i=>i.kind==='flag').length,2);
  await screenshot('level-authored');
  const beforeCancel=await map();await click(50,201);await click(100,250);
  await dragPoints(await at(-10,10),await at(0,10),0,true);assert.deepEqual(await map(),beforeCancel,'Escape cancels a new wall without losing selection safety');
  // Saved custom objects appear in the map palette, without being mutated by placement.
  let asset=structuredClone(originalWall.asset);asset.name='Saved custom block';asset.outline=[[-1,-1],[1,-1],[1,1],[-1,1]];asset.layout=null;asset.pinned_edge=null;
  await evaluate(`localStorage.setItem('fracture.objects.v1',JSON.stringify({objects:[${JSON.stringify(asset)}]}))`);
  await key('KeyM','m',77);await key('KeyM','m',77);await click(50,201);await click(100,611);await mapClick(12,-5);
  assert.equal((await map()).items.at(-1).asset.name,'Saved custom block');
  const savedObjects=await evaluate('localStorage.getItem("fracture.objects.v1")');await field(1200,310,'3');assert.equal(await evaluate('localStorage.getItem("fracture.objects.v1")'),savedObjects);
  // Export uses the browser download boundary, import remains validated and undoable.
  await evaluate('window.levelDownloads=[];window.levelCreate=URL.createObjectURL;URL.createObjectURL=b=>{window.levelDownloads.push(b);return window.levelCreate(b)}');
  await click(490,40);await click(550,180);assert.equal(await evaluate('window.levelDownloads.length'),1);
  const authored=await map();
  await writeFile('/tmp/fracture-level-import.json',JSON.stringify(authored));
  const {root}=await cdp('DOM.getDocument');const {nodeIds}=await cdp('DOM.querySelectorAll',{nodeId:root.nodeId,selector:'input[type=file]'});
  await cdp('DOM.setFileInputFiles',{nodeId:nodeIds.at(-1),files:['/tmp/fracture-level-import.json']});await delay(200);assert.deepEqual(await map(),authored);
  assert.equal(await evaluate('wasm_exports.level_import_buffer(2097153)'),0);
  await importMap({...authored,version:99});assert.deepEqual(await map(),authored);
  await cdp('Page.reload');await until('wasm_exports?.lab_ticks?.()>5','reload');await key('KeyM','m',77);assert.deepEqual(await map(),authored);
  // Playtest keeps the authored recipe; enemy contact wins and R restarts.
  const trial={version:1,name:'Flag race',size:[24,15],items:[
    {kind:'spawn',team:'blue',position:[-7,0],angle:0},{kind:'spawn',team:'red',position:[7,0],angle:Math.PI},
    {kind:'flag',team:'blue',position:[2,0]},{kind:'flag',team:'red',position:[-2,0]}]};
  await importMap(trial);const parsedTrial=await map();await click(1120,40);await until('wasm_exports.level_editor_active()===0 && wasm_exports.lab_flags()===2','play map');
  for (const n of [5,6,7,8]){await key('Digit'+n,String(n),48+n);await click(640,441);await delay(500);assert.equal(await evaluate('wasm_exports.lab_winner()'),0);}
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.lab_winner()===1','blue capture');await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});await screenshot('level-blue-victory');
  await key('KeyR','r',82);await until('wasm_exports.lab_winner()===0 && wasm_exports.lab_tank_x()<-6.9','restart');
  await key('KeyM','m',77);assert.deepEqual(await map(),parsedTrial);await click(950,40);await click(1120,40);
  await until('wasm_exports.lab_tank_x()>6.9','red spawn');await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.lab_winner()===2','red capture');await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});await screenshot('level-red-victory');
  await key('KeyM','m',77);await importMap(authored);
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:2,mobile:false});await delay(300);await screenshot('level-high-dpi');
  await evaluate('localStorage.setItem("fracture.level.v1","invalid saved map")');await cdp('Page.reload');await until('wasm_exports?.lab_ticks?.()>5','invalid draft startup');await key('KeyM','m',77);
  await click(110,250);await mapDrag([-8,-5],[8,-5]);assert.equal(await evaluate('localStorage.getItem("fracture.level.v1")'),'invalid saved map','unreadable saved data is preserved');
  assert.deepEqual(errors,[]);console.log(JSON.stringify({result:'PASS',checks:['wall drawing','dimensions','rotation','vertices','undo redo','cancel drag','terrain','bridge','fixed obstacle','team markers','saved library','export import','file picker','import limits','draft reload','unreadable draft preservation','weapons ignore flags','both team victories','restart','high DPI']},null,2));
} catch(error) {await screenshot("level-failure");console.log("DRAFT",JSON.stringify(await map()));throw error;} finally {
  if(original) await evaluate(`(()=>{for(const [key,value]of Object.entries({'fracture.level.v1':${JSON.stringify(original.level)},'fracture.objects.v1':${JSON.stringify(original.objects)}})){if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value);}})()`).catch(()=>{});
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});ws.close();
}
