// Isolated Chrome must be started with --mute-audio and a fresh profile.
import assert from 'node:assert/strict';
import {writeFile,mkdir} from 'node:fs/promises';
const debug=process.env.CHROME_DEBUG_URL||'http://127.0.0.1:9254';
const url=process.argv[2];assert(url,'Pass a replay URL');
const page=await fetch(`${debug}/json/new?${encodeURIComponent(url)}`,{method:'PUT'}).then(r=>r.json());
const ws=new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
let seq=0;const pending=new Map(),errors=[];
ws.onmessage=({data})=>{const message=JSON.parse(data);if(message.method==='Runtime.exceptionThrown')errors.push(message.params);if(message.id){const p=pending.get(message.id);pending.delete(message.id);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result)}};
const cdp=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}))});
async function evaluate(expression){const r=await cdp('Runtime.evaluate',{expression,returnByValue:true});if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;}
const delay=ms=>new Promise(r=>setTimeout(r,ms));
await cdp('Runtime.enable');await cdp('Page.enable');
await cdp('Emulation.setDeviceMetricsOverride',{width:1440,height:1100,deviceScaleFactor:1,mobile:false});
let loaded=false;
for(let i=0;i<120;i++){if(await evaluate('Number(document.querySelector("#seek")?.max)>0')){loaded=true;break}await delay(500)}
assert(loaded,'Replay did not load');assert.deepEqual(errors,[]);
await mkdir('target/screenshots',{recursive:true});
for(const view of [-1,0,1]){
 await evaluate(`document.querySelector('#seek').value=Math.floor(Number(document.querySelector('#seek').max)/2);document.querySelector('#seek').dispatchEvent(new Event('input'));document.querySelector('#view').value=${view};document.querySelector('#view').dispatchEvent(new Event('change'));`);
 await delay(100);
 const {data}=await cdp('Page.captureScreenshot',{format:'png'});await writeFile(`target/screenshots/ai-latest-view-${view}.png`,Buffer.from(data,'base64'));
}
await evaluate(`document.querySelector('#play').click()`);await delay(350);await evaluate(`document.querySelector('#play').click();document.querySelector('#seek').value=document.querySelector('#seek').max;document.querySelector('#seek').dispatchEvent(new Event('input'))`);
const status=await evaluate('document.querySelector("#status").textContent');assert.equal(status,'240.0 / 240.0 s');assert.deepEqual(errors,[]);
console.log(JSON.stringify({status,views:3,seek:true,playback:true,errors:errors.length}));
ws.close();await fetch(`${debug}/json/close/${page.id}`);
