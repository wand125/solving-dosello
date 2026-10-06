// Local-only Chromium smoke through a debugging pipe; no npm dependencies.
import {spawn} from 'node:child_process';
import {mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import assert from 'node:assert/strict';
const [executable,base]=process.argv.slice(2);
if(!executable||!base)throw Error('Usage: node test/browser-smoke.js <chromium executable> <local server URL>');
const url=new URL(base);if(url.hostname!=='[::1]')throw Error('Use IPv6 loopback for the local server');
const profile=mkdtempSync(join(tmpdir(),'solving-browser-'));
const blockedProxy=new URL(url);blockedProxy.port='9';
const chrome=spawn(executable,['--headless=new','--remote-debugging-pipe','--disable-background-networking','--disable-component-update','--disable-sync','--no-first-run','--no-default-browser-check','--disable-extensions','--disable-features=MediaRouter,OptimizationHints','--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE [::1], EXCLUDE ::1','--proxy-server='+blockedProxy.origin,'--proxy-bypass-list=[::1]','--user-data-dir='+profile,'about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
let seq=0,buffer='',session;const pending=new Map(),errors=[];
chrome.stdio[4].on('data',chunk=>{buffer+=chunk.toString();let at;while((at=buffer.indexOf('\0'))>=0){const message=JSON.parse(buffer.slice(0,at));buffer=buffer.slice(at+1);if(message.id){const p=pending.get(message.id);pending.delete(message.id);if(message.error)p?.reject(Error(JSON.stringify(message.error)));else p?.resolve(message.result);}else if(message.method==='Runtime.exceptionThrown')errors.push(message.params.exceptionDetails.text);else if(message.method==='Log.entryAdded'&&message.params.entry.level==='error')errors.push(message.params.entry.text);}});
function call(method,params={},sid=session){return new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout: '+method));},20000);pending.set(id,{resolve:r=>{clearTimeout(timer);resolve(r);},reject:e=>{clearTimeout(timer);reject(e);}});chrome.stdio[3].write(JSON.stringify({id,method,params,...(sid?{sessionId:sid}:{})})+'\0');});}
async function evaluate(expression){const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;}
async function until(expression){for(let i=0;i<100;i++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,100));}throw Error('Condition failed: '+expression+'; '+JSON.stringify({errors,page:await evaluate("document.body.innerText.slice(0,1800)")}));}
async function navigate(path){await call('Page.navigate',{url:new URL(path,url).href});await until("document.readyState==='complete'");}
try{
 const target=await call('Target.createTarget',{url:'about:blank'},null);session=(await call('Target.attachToTarget',{targetId:target.targetId,flatten:true},null)).sessionId;
 await call('Page.enable');await call('Runtime.enable');await call('Log.enable');
 await navigate('play/?lang=en');await until("document.querySelectorAll('#moves tr').length===20 && document.querySelector('#position-value').textContent==='Exact +2'");
 assert.equal(await evaluate("document.documentElement.lang"),'en');
 assert.match(await evaluate("[...document.querySelectorAll('#moves tr')].find(r=>r.querySelector('button').textContent==='f3-f4').textContent"),/\+2Exact/);
 assert.equal(await evaluate("document.querySelectorAll('#board rect[stroke=\"#d4a900\"]').length"),2);
 assert.deepEqual(await evaluate("[...document.querySelectorAll('#moves tr.best button')].map(x=>x.textContent).sort()"),['c5-c6','f3-f4']);
 await evaluate("document.querySelector('#mode').value='analysis';document.querySelector('#mode').dispatchEvent(new Event('change'));document.querySelector('#sequence').value='f3-f4 e6-f6 d7-e7';document.querySelector('#apply').click()");await until("document.querySelectorAll('#record button').length===4");
 await evaluate("document.querySelector('#undo').click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4 e6-f6');
 await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}))");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4 e6-f6 d7-e7');
 await evaluate("document.querySelectorAll('#record button')[1].click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4');
 await evaluate("document.querySelector('[data-language=ja]').click()");assert.equal(await evaluate('document.documentElement.lang'),'ja');assert.equal(await evaluate('document.title'),'DOSELLO 解析ツール');
 await navigate('');await until("document.documentElement.lang==='ja'");assert.match(await evaluate("document.querySelector('#strength').textContent"),/194勝/);
 await navigate('?lang=en');await until("document.documentElement.lang==='en'");assert.match(await evaluate("document.querySelector('#strength').textContent"),/194 wins/);
 await evaluate("document.querySelector('#pv-next').click()");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/Ply 1/);
 await evaluate("document.querySelector('[data-language=ja]').click()");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/1手目/);
 await evaluate("document.querySelector('#pv-last').click()");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/黒 30・白 28/);
 await evaluate("document.querySelector('[data-language=en]').click()");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/Terminal: Black wins by 2/);
 assert.equal(await evaluate("document.querySelector('#pv-next').disabled"),true);
 await evaluate("document.querySelector('#pv-prev').click()");assert.equal(await evaluate("document.querySelector('#pv-step').value"),'24');
 await evaluate("document.querySelector('#pv-step').value='12';document.querySelector('#pv-step').dispatchEvent(new Event('input'))");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/Ply 12: a3-b3/);
 await evaluate("document.querySelector('#pv-first').click()");assert.match(await evaluate("document.querySelector('#pv-caption').textContent"),/Initial position/);
 await call('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
 await call('Emulation.clearDeviceMetricsOverride');
 await navigate('play/?lang=en');await until("document.querySelectorAll('#moves tr.best').length===2");
 await evaluate("[...document.querySelectorAll('#moves button')].find(b=>b.textContent==='f3-f4').click()");await until("document.querySelectorAll('#record button').length>=3");
 await evaluate("document.querySelector('#undo').click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 await evaluate("document.querySelector('#mode').value='white';document.querySelector('#mode').dispatchEvent(new Event('change'))");await until("document.querySelector('#sequence').value.length>0");
 await call('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
 assert(await evaluate("document.querySelector('aside').getBoundingClientRect().top>=document.querySelector('.board-area').getBoundingClientRect().bottom"));
 await call('Emulation.setScriptExecutionDisabled',{value:true});await navigate('?lang=en');assert.equal(await evaluate('document.documentElement.lang'),'en');assert.match(await evaluate("document.querySelector('#strength').textContent"),/194 wins/);
 assert.deepEqual(errors.filter(e=>!e.includes('favicon')),[]);
 console.log('PASS browser: CSP/WASM/Worker, 20 moves, book +2, best outlines, sequence, undo/redo, record jump, both AI colors, language persistence/override, translated PV and mobile layout');
}finally{await call('Browser.close',{},null).catch(()=>{});chrome.kill();await new Promise(r=>setTimeout(r,500));rmSync(profile,{recursive:true,force:true});}
