// Node 22+; no npm dependencies. Requires a local server and Chrome with
// --mute-audio --remote-debugging-port=9227. Takes about 3.5 minutes.
// Screenshots go to target/screenshots/.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';

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
async function until(expression, label, attempts = 160) {
  for (let i = 0; i < attempts; i++) {
    if (await evaluate(expression)) return;
    await delay(50);
  }
  throw new Error(`Timed out: ${label}`);
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
async function importMap(level){const json=JSON.stringify(level);await evaluate(`(()=>{const bytes=new TextEncoder().encode(${JSON.stringify(json)});const ptr=wasm_exports.level_import_buffer(bytes.length);new Uint8Array(wasm_memory.buffer,ptr,bytes.length).set(bytes);wasm_exports.level_import_commit();})()`);await delay(200);}

try {
  await mkdir('target/screenshots', {recursive:true});
  await cdp('Runtime.enable'); await cdp('Page.enable');
  await cdp('Network.enable'); await cdp('Network.setCacheDisabled', {cacheDisabled:true});
  await cdp('Emulation.setDeviceMetricsOverride', {width:1440,height:900,deviceScaleFactor:1,mobile:false});
  await cdp('Page.reload');
  await until('typeof wasm_exports!=="undefined" && wasm_exports?.game_ready?.()===1 && document.getElementById("status").hidden', 'startup');
  await click(260,512);
  await until('wasm_exports.game_screen()===1', 'lobby');
  assert.equal(await evaluate('wasm_exports.game_airdrops_enabled()'), 1);
  await click(220,596);
  await until('wasm_exports.game_airdrops_enabled()===0', 'desktop disable');
  await screenshot('airdrop-desktop-disabled');
  await click(220,596);
  await until('wasm_exports.game_airdrops_enabled()===1', 'desktop enable');
  await cdp('Emulation.setDeviceMetricsOverride', {width:1280,height:800,deviceScaleFactor:1,mobile:false});
  await delay(300);
  await click(1090,615);
  await until('wasm_exports.game_airdrops_enabled()===0', 'Deck disable');
  await screenshot('airdrop-deck-disabled');
  await click(1090,615);
  await until('wasm_exports.game_airdrops_enabled()===1', 'Deck enable');
  // The river prevents a capture while the real match clock reaches 3 minutes.
  await importMap({version:1,name:'Airdrop timing check',size:[48,30],enemy_activity:'static',items:[
    {kind:'spawn',team:'blue',position:[-18,0],angle:0},
    {kind:'spawn',team:'red',position:[18,0],angle:Math.PI},
    {kind:'flag',team:'blue',position:[-21,0]},
    {kind:'flag',team:'red',position:[21,0]},
    {kind:'terrain',terrain:'Water',min:[-2,-14.7],max:[2,14.7]}
  ]});
  await until('JSON.parse(localStorage.getItem("fracture.match-map.v1") || "null")?.name==="Airdrop timing check"', 'imported test arena');
  await click(1100,705);
  await until('wasm_exports.game_phase()===2', 'match');
  assert.equal(await evaluate('wasm_exports.game_airdrop_remaining()'), 0);
  console.log('PASS: desktop and Deck toggle; waiting for the real three-minute match timer');
  await until('wasm_exports.game_match_ticks()>=21550', 'three-minute timer', 6000);
  assert.equal(await evaluate('wasm_exports.game_airdrop_remaining()'), 0, 'no early drop');
  await until('wasm_exports.game_airdrop_remaining()>0', 'first drop');
  const spawned=await evaluate('({ticks:wasm_exports.game_match_ticks(),remaining:wasm_exports.game_airdrop_remaining()})');
  assert.ok(spawned.ticks>=21600 && spawned.ticks<21630, JSON.stringify(spawned));
  assert.ok(spawned.remaining>29 && spawned.remaining<=30, JSON.stringify(spawned));
  await screenshot('airdrop-deck-active');
  console.log('PASS: first drop', JSON.stringify(spawned));
  await until('wasm_exports.game_airdrop_remaining()===0', 'expiry', 1000);
  const expired=await evaluate('wasm_exports.game_match_ticks()');
  assert.ok(expired>=25200 && expired<25230, String(expired));
  await screenshot('airdrop-deck-expired');
  assert.equal(errors.length,0,JSON.stringify(errors));
  console.log(JSON.stringify({result:'PASS',spawned,expired,checks:['desktop toggle','Deck toggle','three-minute schedule','30-second expiry','public marker']},null,2));
} finally {
  ws.close();
  await fetch(`${debugUrl}/json/close/${page.id}`).catch(()=>{});
}
