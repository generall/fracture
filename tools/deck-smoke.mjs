// Standard Gamepad API fixture; no physical controller or npm dependencies.
// Node 22+. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.GAME_URL || 'http://127.0.0.1:8000');
sandboxUrl.searchParams.delete('mode');
sandboxUrl.searchParams.set('deck','1');
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


const hash = s => [...s].reduce((h,c) => Math.imul(h ^ c.charCodeAt(0),16777619) >>> 0,2166136261);
const focus = () => evaluate('wasm_exports.controller_focus_label() >>> 0');
async function button(index, ms=90) {
  await evaluate(`pad.buttons[${index}]={pressed:true,value:1}`); await delay(ms);
  await evaluate(`pad.buttons[${index}]={pressed:false,value:0}`); await delay(90);
}
async function seek(label) {
  // Explore the actual directional graph; never activate by coordinates.
  const target=hash(label), visited=new Set();
  async function walk(depth=0) {
    let here=await focus(); if(here===target) return true;
    const position=await evaluate('[wasm_exports.controller_focus_x(),wasm_exports.controller_focus_y()].join()');
    if(visited.has(position)||depth>50) return false;
    visited.add(position);
    for(const direction of [13,15,12,14]) {
      await button(direction); const next=await focus();
      if(await walk(depth+1)) return true;
      if(next!==here) {
        // Navigation need not be symmetric, so reload the path by searching
        // from the current node; all menus must still be connected.
        here=await focus();
      }
    }
    return false;
  }
  if(!await walk()) throw new Error(`Controller cannot reach ${label}; focus=${await focus()}`);
}
let originalStores;
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Network.enable'); await cdp('Network.setCacheDisabled',{cacheDisabled:true});
  await cdp('Page.addScriptToEvaluateOnNewDocument',{source:`
    window.pad={connected:true,index:0,mapping:'standard',axes:[0,0,0,0],buttons:Array.from({length:17},()=>({pressed:false,value:0}))};
    window.padConnected=true;
    Object.defineProperty(navigator,'getGamepads',{value:()=>padConnected?[pad]:[]});
  `});
  for (const height of [800,720]) {
    await cdp('Emulation.setDeviceMetricsOverride',{width:1280,height,deviceScaleFactor:1,mobile:false});
    await cdp('Page.reload');
    await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1','startup');
    if(!originalStores) originalStores=await evaluate('Object.fromEntries(["fracture.match-map.v1","fracture.audio.v1"].map(k=>[k,localStorage.getItem(k)]))');
    await delay(200); await button(13);
    assert.equal(await evaluate('wasm_exports.controller_active()'),1);
    assert.equal(await evaluate("getComputedStyle(document.getElementById('glcanvas')).cursor"),'none','controller hides system cursor');
    await screenshot(`deck-home-${height}`);
    await seek('LOCAL MATCH'); await button(0);
    await until('wasm_exports.game_screen()===1','controller enters lobby');
    await screenshot(`deck-lobby-${height}`);
    await seek('CLASSIC TANKS 2'); await button(0);
    await screenshot(`deck-classic-tanks-2-${height}`);
    await button(3); await delay(200);
    await screenshot(`deck-settings-${height}`);
    await seek('TEST EFFECT'); await button(0);
    await button(1); await delay(200);
    await seek('START MATCH'); await button(0);
    await until('wasm_exports.game_phase()===1','countdown');
    await button(7); assert.equal((await stats()).shots,0,'trigger blocked during countdown');
    await until('wasm_exports.game_phase()===2','running');
    await evaluate('pad.axes[2]=0;pad.axes[3]=-1'); await delay(1800);
    const angle=await evaluate('wasm_exports.lab_turret_angle()');
    assert.ok(Math.abs(Math.atan2(Math.sin(angle+Math.PI/2),Math.cos(angle+Math.PI/2)))<0.15,`right stick aims up: ${angle}`);
    await evaluate('pad.axes[3]=0'); await delay(200);
    assert.ok(Math.abs(await evaluate('wasm_exports.lab_turret_angle()')-angle)<0.05,'released stick holds aim');
    await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1180,y:40}); await delay(200);
    assert.equal(await evaluate('wasm_exports.controller_active()'),1,'emulated mouse motion cannot steal controller mode');
    assert.equal(await focus(),0,'live HUD has no controller focus');
    assert.ok(Math.abs(await evaluate('wasm_exports.lab_turret_angle()')-angle)<0.05,'mouse motion after stick release cannot redirect turret');
    const start=await stats();
    await evaluate('pad.axes[1]=-0.7'); await delay(500); await evaluate('pad.axes[1]=0');
    const end=await stats(); assert.ok(Math.hypot(end.x-start.x,end.y-start.y)>0.1,'analog drive moves tank');
    assert.equal(await focus(),0,'left stick drives without navigating HUD buttons');
    const drivingAmmo=await evaluate('wasm_exports.lab_ammunition()');
    await button(0);
    assert.equal(await evaluate('wasm_exports.game_phase()'),2,'A cannot activate the live HUD menu');
    assert.equal(await evaluate('wasm_exports.lab_ammunition()'),drivingAmmo,'A cannot activate HUD ammo buttons');
    await button(7); assert.ok((await stats()).shots>start.shots,'RT fires');
    const ammo=await evaluate('wasm_exports.lab_ammunition()'); await button(5);
    assert.notEqual(await evaluate('wasm_exports.lab_ammunition()'),ammo,'RB changes ammo');
    await button(2); await screenshot(`deck-help-${height}`); await button(2);
    await screenshot(`deck-match-${height}`);
    await button(9); await until('wasm_exports.game_phase()===3','Start pauses');
    const paused=await stats();
    const menuFocus=await focus();
    await evaluate('pad.axes[1]=1'); await delay(150); await evaluate('pad.axes[1]=0'); await delay(100);
    assert.notEqual(await focus(),menuFocus,'left stick navigates the open pause menu');
    assert.equal((await stats()).x,paused.x,'menu navigation does not drive the tank');
    await button(7); await delay(150);
    assert.equal((await stats()).shots,paused.shots,'pause blocks firing');
    assert.equal((await stats()).ticks,paused.ticks,'pause stops simulation');
    await screenshot(`deck-pause-${height}`);
    await button(1); await until('wasm_exports.game_phase()===2','B resumes');
    await evaluate('pad.axes[1]=-1;pad.buttons[7]={pressed:true,value:1}'); await delay(100);
    await evaluate('padConnected=false'); await until('wasm_exports.game_phase()===3','disconnect opens pause');
    const disconnected=await stats(); await delay(200); assert.equal((await stats()).ticks,disconnected.ticks);
    await evaluate('padConnected=true'); await delay(200);
    assert.equal((await stats()).shots,disconnected.shots,'reconnect with held trigger does not fire');
    await evaluate('pad.axes[1]=0;pad.buttons[7]={pressed:false,value:0}'); await delay(200);
    await button(1); await until('wasm_exports.game_phase()===2','neutral reconnect can resume');
    await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1000,y:400}); await delay(200);
    assert.equal(await evaluate('wasm_exports.controller_active()'),1,'idle pad stays active through mouse motion');
    assert.equal(await evaluate("getComputedStyle(document.getElementById('glcanvas')).cursor"),'none');
    await click(1000,400);
    assert.equal(await evaluate('wasm_exports.controller_active()'),1,'emulated left click retains controller aim');
    await cdp('Input.dispatchMouseEvent',{type:'mousePressed',x:1000,y:400,button:'middle',clickCount:1});
    await delay(60);
    await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',x:1000,y:400,button:'middle',clickCount:1});
    await delay(60);
    assert.equal(await evaluate('wasm_exports.controller_active()'),0,'middle click restores mouse controls');
    assert.notEqual(await evaluate("getComputedStyle(document.getElementById('glcanvas')).cursor"),'none','mouse handoff restores cursor');
    await button(9); await seek('RETURN TO LOBBY'); await button(0);
    await until('wasm_exports.game_screen()===1','controller returns to lobby');
    await button(1); await until('wasm_exports.game_screen()===0','B returns home');
  }
  const quick={version:1,name:'Controller weapon/results check',size:[48,30],enemy_activity:'static',items:[
    {kind:'spawn',team:'blue',position:[-18,0],angle:0},
    {kind:'spawn',team:'red',position:[18,0],angle:Math.PI},
    {kind:'flag',team:'blue',position:[-21,0]},
    {kind:'flag',team:'red',position:[-14,0]},
    {kind:'pickup',weapon:'laser',position:[-18,0],respawn_time:20,charges:2,cooldown:2}
  ]};
  await evaluate(`localStorage.setItem('fracture.match-map.v1',${JSON.stringify(JSON.stringify(quick))})`);
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1','fixture startup');
  await delay(200); await button(13); await seek('LOCAL MATCH'); await button(0);
  await until('wasm_exports.game_screen()===1','fixture lobby');
  await seek('IMPORTED MAP'); await button(0); await seek('START MATCH'); await button(0);
  await until('wasm_exports.game_phase()===2 && wasm_exports.lab_special()===1','laser collected');
  await screenshot('deck-special-ready-720');
  await button(9); await button(6);
  assert.equal(await evaluate('wasm_exports.lab_special()'),1,'pause blocks special weapon');
  await button(1); await button(6);
  await until('wasm_exports.lab_laser_remaining()>0','LT activates collected weapon');
  await screenshot('deck-special-active-720');
  await until('wasm_exports.lab_laser_remaining()===0','laser burst finishes');
  await screenshot('deck-special-reload-720');
  await delay(2200); await button(6);
  await until('wasm_exports.lab_laser_remaining()>0 && wasm_exports.lab_special()===0','last charge remains active');
  await screenshot('deck-special-last-charge-720');
  await until('wasm_exports.lab_laser_remaining()===0','last burst finishes');
  await screenshot('deck-special-empty-720');
  await evaluate('pad.axes[0]=1');
  await until('wasm_exports.game_phase()===4','controller reaches enemy flag');
  await evaluate('pad.axes[0]=0'); await delay(150);
  await screenshot('deck-results-720');
  await seek('REMATCH'); await button(0);
  await until('wasm_exports.game_phase()===1','controller rematch');
  await button(9); await seek('RETURN TO LOBBY'); await button(0);
  await until('wasm_exports.game_screen()===1','leave rematch');
  await button(1); await until('wasm_exports.game_screen()===0','home before docking');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1920,height:1080,deviceScaleFactor:1,mobile:false});
  await delay(250); await screenshot('deck-docked-home-1080');
  await seek('ENTER LOCAL LOBBY  >'); await button(0);
  await until('wasm_exports.game_screen()===1','docked controller lobby');
  await screenshot('deck-docked-lobby-1080');
  await seek('START MATCH  >');
  // The handheld fixes are opt-in: ordinary desktop gamepad behavior stays intact.
  const desktopUrl=new URL(url); desktopUrl.searchParams.delete('deck');
  await cdp('Page.navigate',{url:desktopUrl.href});
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1','desktop startup');
  await delay(200); await button(13);
  assert.equal(await evaluate('wasm_exports.controller_active()'),1,'desktop still accepts gamepad input');
  assert.notEqual(await evaluate("getComputedStyle(document.getElementById('glcanvas')).cursor"),'none','desktop cursor stays visible');
  await cdp('Input.dispatchMouseEvent',{type:'mouseMoved',x:1000,y:400}); await delay(200);
  assert.equal(await evaluate('wasm_exports.controller_active()'),0,'desktop retains mouse-motion handoff');
  assert.deepEqual(errors,[]);
  console.log('PASS: 1280x800/720 controller menus, settings, analog drive/aim, triggers, ammo, special weapons, results/rematch, pause, disconnect/reconnect and mouse handoff');
} catch(e) {
  await screenshot('deck-error').catch(()=>{}); console.error(errors); console.error(await evaluate('({url:location.href,deck:importObject.env.controller_handheld_mode?.(),cursor:canvas.style.cursor,active:wasm_exports.controller_active()})')); throw e;
} finally {
  if(originalStores) await evaluate(`Object.entries(${JSON.stringify(originalStores)}).forEach(([k,v])=>v===null?localStorage.removeItem(k):localStorage.setItem(k,v))`);
  await fetch(`${debugUrl}/json/close/${page.id}`); ws.close();
}
