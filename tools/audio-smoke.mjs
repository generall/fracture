// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --remote-debugging-port=9227. Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile, readFile } from 'node:fs/promises';

const debugUrl = process.env.CHROME_DEBUG_URL || 'http://127.0.0.1:9227';
const sandboxUrl = new URL(process.env.GAME_URL || 'http://127.0.0.1:8000');
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

async function importMap(level) {
  const json=JSON.stringify(level);
  await evaluate(`(()=>{const bytes=new TextEncoder().encode(${JSON.stringify(json)});const ptr=wasm_exports.level_import_buffer(bytes.length);new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);wasm_exports.level_import_commit();})()`);
  await until(`JSON.parse(localStorage.getItem('fracture.match-map.v1')||'null')?.name===${JSON.stringify(level.name)}`,'audio fixture accepted');
}
let stores;
try {
  await mkdir('target/screenshots',{recursive:true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports.game_ready?.()===1','audio and game ready');
  stores=await evaluate('Object.fromEntries(["fracture.audio.v1","fracture.match-map.v1"].map(k=>[k,localStorage.getItem(k)]))');
  await evaluate('localStorage.removeItem("fracture.audio.v1")');
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports.game_ready?.()===1','clean audio settings');
  assert.equal(await evaluate('fractureAudio.loaded'),32);
  assert.equal(await evaluate('fractureAudio.errors'),0);
  assert.equal(await evaluate('wasm_exports.audio_battle_mix()'),0);
  await click(260,512);
  await until('fractureAudio.context().state==="running"','gesture unlock');
  await until('fractureAudio.rms()>0.001','music reaches output');
  const fixture={version:1,name:'Audio sight test',size:[48,30],enemy_activity:'static',items:[
    {kind:'spawn',team:'blue',position:[-8,0],angle:0},
    {kind:'spawn',team:'red',position:[8,0],angle:Math.PI},
    {kind:'flag',team:'blue',position:[-21,0]},
    {kind:'flag',team:'red',position:[21,0]},
    {kind:'terrain',terrain:'Road',min:[-23,-14],max:[23,14]},
    {kind:'terrain',terrain:'Sand',min:[-6,-14],max:[23,14]}
  ]};
  const scenery=JSON.parse(await readFile(new URL('../levels/river-divide.json',import.meta.url),'utf8'));
  for (const [kind,x] of [['shrub',-6],['crate',-4.2],['timber',-2]]) {
    const prop=scenery.items.find(item=>item.asset?.components?.decoration===kind);
    assert.ok(prop,`bundled ${kind} fixture`);
    fixture.items.push({...prop,position:[x,0],angle:0});
  }
  await importMap(fixture); await click(1250,837);
  await until('wasm_exports.game_phase()===2','match starts');
  await until('wasm_exports.audio_enemy_visible()===1','visible enemy triggers music');
  const transition=await evaluate('wasm_exports.audio_battle_mix()');
  assert.ok(transition<1,'transition is gradual');
  await until('wasm_exports.audio_battle_mix()>0.9','battle arrangement fades in');
  await key('F7','F7',118);
  await until('wasm_exports.audio_settings_open()===1','settings during live play');
  const liveFrozen=await evaluate('wasm_exports.game_match_ticks()');
  const liveShots=await evaluate('wasm_exports.lab_shots()');
  await key('KeyF','f',70);await delay(100);
  assert.equal(await evaluate('wasm_exports.game_match_ticks()'),liveFrozen);
  assert.equal(await evaluate('wasm_exports.lab_shots()'),liveShots);
  await key('Escape','Escape',27);
  await until('wasm_exports.audio_settings_open()===0 && wasm_exports.game_phase()===2','settings returns to live play');
  await until(`wasm_exports.game_match_ticks()>${liveFrozen}`,'match resumes after settings');
  await evaluate('window.decorSounds=new Set();window.decorWatch=setInterval(()=>{for(const v of fractureAudio.voices())if(v.sound>=30&&v.sound<=32&&!v.loop&&v.gain>0)decorSounds.add(v.sound)},10)');
  await cdp('Input.dispatchKeyEvent',{type:'keyDown',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  await until('wasm_exports.audio_engine_gain()>0.06','engine rises with throttle');
  await until('fractureAudio.voices().some(v=>v.sound===26 && v.loop && v.gain>0.002)','road texture follows movement');
  assert.ok(await evaluate('fractureAudio.voices().filter(v=>v.sound>=27 && v.sound<=29).every(v=>v.gain<0.0001)'),'other terrain textures stay silent');
  await until('wasm_exports.lab_tank_x()>-6 && fractureAudio.voices().some(v=>v.sound===27 && v.gain>0.002)','crossing onto sand changes movement sound');
  await until('fractureAudio.voices().filter(v=>v.sound===26).every(v=>v.gain<0.0005)','road texture fades out after crossing');
  await until('decorSounds.size===3','running over shrub, crate and timber plays their material cues');
  await evaluate('clearInterval(decorWatch)');
  await cdp('Input.dispatchKeyEvent',{type:'keyUp',code:'KeyW',key:'w',windowsVirtualKeyCode:87});
  const before=await evaluate('wasm_exports.audio_effects_played()');
  await key('KeyF','f',70);
  await until(`wasm_exports.audio_effects_played()>${before}`,'shot sound');
  await key('Escape','Escape',27); await delay(500);
  assert.equal(await evaluate('wasm_exports.audio_engine_gain()'),0,'pause silences engine');
  assert.ok(await evaluate('fractureAudio.voices().filter(v=>v.sound>=26 && v.sound<=29).every(v=>v.gain<0.0001)'),'pause silences terrain movement');
  const paused=await evaluate('wasm_exports.audio_battle_mix()');
  await delay(300); assert.equal(await evaluate('wasm_exports.audio_battle_mix()'),paused,'pause holds music state');
  await screenshot('audio-pause-controls');
  await click(1285,880);
  await until('wasm_exports.audio_settings_open()===1','sound settings button');
  const frozen=await evaluate('wasm_exports.game_match_ticks()');
  await key('KeyF','f',70); await key('KeyN','n',78);
  await click(853,373); // master 75%
  await click(667,438); // music 40%
  await click(880,503); // effects 80%
  await click(560,568); // feedback 20%
  const prefs=await evaluate('Number(localStorage.getItem("fracture.audio.v1"))');
  assert.equal((prefs>>1)&127,75);assert.equal((prefs>>8)&127,40);
  assert.equal((prefs>>15)&127,80);assert.equal((prefs>>22)&127,20);
  assert.equal(await evaluate('wasm_exports.game_match_ticks()'),frozen,'settings blocks gameplay');
  await screenshot('audio-settings');
  await click(780,647); // Preview a collision through the effects mixer.
  await key('Escape','Escape',27);
  await until('wasm_exports.audio_settings_open()===0','close settings');
  assert.equal(await evaluate('wasm_exports.game_phase()'),3,'closing settings preserves pause');
  await key('F8','F8',119); await delay(250);
  assert.equal(await evaluate('wasm_exports.audio_muted()'),1);
  assert.ok(await evaluate('fractureAudio.rms()<0.0001'),'mute reaches actual output');
  await key('F9','F9',120); await key('F10','F10',121);
  const beforeClick=await evaluate('Number(localStorage.getItem("fracture.audio.v1"))');
  await click(700,883); // Music has its own visible hit target.
  assert.equal(await evaluate('(Number(localStorage.getItem("fracture.audio.v1"))>>8)&127'),20);
  assert.equal(await evaluate('Number(localStorage.getItem("fracture.audio.v1"))&1'),beforeClick&1);
  const settings=await evaluate('localStorage.getItem("fracture.audio.v1")');
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports.game_ready?.()===1','reload muted');
  assert.equal(await evaluate('wasm_exports.audio_muted()'),1,'mute restored');
  assert.equal(await evaluate('localStorage.getItem("fracture.audio.v1")'),settings);
  await key('F8','F8',119); await click(260,512);
  // Solid cover must suppress the music trigger, regardless of enemy proximity.
  fixture.items.push({kind:'solid',position:[0,0],angle:0,outline:[[-1,-14],[1,-14],[1,14],[-1,14]]});
  await importMap(fixture); await click(1250,837);
  await until('wasm_exports.game_phase()===2','occluded match starts');
  await delay(500);
  assert.equal(await evaluate('wasm_exports.audio_enemy_visible()'),0);
  assert.equal(await evaluate('wasm_exports.audio_battle_mix()'),0);
  assert.ok(await evaluate('fractureAudio.active<=32'),'voice budget');
  assert.deepEqual(errors,[]);
  console.log('PASS: 32 decoded assets, gesture unlock, audible music, visible/occluded enemy music, smooth transition, terrain movement/shot/decorative destruction cues, pause, modal sliders, input isolation, mute output and saved mixer settings');
} catch(error) {
  console.error('Audio smoke state:',await stats(),await evaluate('({phase:wasm_exports.game_phase(),visible:wasm_exports.audio_enemy_visible(),mix:wasm_exports.audio_battle_mix(),errors:fractureAudio.errors})'));
  await screenshot('audio-failure');
  throw error;
} finally {
  if(stores) await evaluate(`Object.entries(${JSON.stringify(stores)}).forEach(([k,v])=>v===null?localStorage.removeItem(k):localStorage.setItem(k,v))`);
  await fetch(`${debugUrl}/json/close/${page.id}`); ws.close();
}
