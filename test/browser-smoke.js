// Local-only Chromium smoke through a debugging pipe; no npm dependencies.
import {spawn} from 'node:child_process';
import {mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import assert from 'node:assert/strict';
import {perfectLine} from '../docs/perfect-line.js';
const [executable,base]=process.argv.slice(2);
if(!executable||!base)throw Error('Usage: node test/browser-smoke.js <chromium executable> <local server URL>');
const url=new URL(base);if(url.hostname!=='[::1]')throw Error('Use IPv6 loopback for the local server');
const profile=mkdtempSync(join(tmpdir(),'solving-browser-'));
const blockedProxy=new URL(url);blockedProxy.port='9';
const chrome=spawn(executable,['--headless=new','--remote-debugging-pipe','--disable-background-networking','--disable-component-update','--disable-sync','--no-first-run','--no-default-browser-check','--disable-extensions','--disable-features=MediaRouter,OptimizationHints','--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE [::1], EXCLUDE ::1','--proxy-server='+blockedProxy.origin,'--proxy-bypass-list=[::1]','--user-data-dir='+profile,'about:blank'],{stdio:['ignore','ignore','ignore','pipe','pipe']});
const deadline=setTimeout(()=>{chrome.kill('SIGKILL');process.exitCode=1;},180000);
let seq=0,buffer='',session;const pending=new Map(),errors=[];
chrome.stdio[4].on('data',chunk=>{buffer+=chunk.toString();let at;while((at=buffer.indexOf('\0'))>=0){const message=JSON.parse(buffer.slice(0,at));buffer=buffer.slice(at+1);if(message.id){const p=pending.get(message.id);pending.delete(message.id);if(message.error)p?.reject(Error(JSON.stringify(message.error)));else p?.resolve(message.result);}else if(message.method==='Runtime.exceptionThrown')errors.push(message.params.exceptionDetails.text);else if(message.method==='Log.entryAdded'&&message.params.entry.level==='error')errors.push(message.params.entry.text);}});
function call(method,params={},sid=session){return new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout: '+method));},20000);pending.set(id,{resolve:r=>{clearTimeout(timer);resolve(r);},reject:e=>{clearTimeout(timer);reject(e);}});chrome.stdio[3].write(JSON.stringify({id,method,params,...(sid?{sessionId:sid}:{})})+'\0');});}
async function evaluate(expression){const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;}
async function until(expression,attempts=100){for(let i=0;i<attempts;i++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,100));}throw Error('Condition failed: '+expression+'; '+JSON.stringify({errors,page:await evaluate("document.body.innerText.slice(0,1800)")}));}
async function navigate(path){await call('Page.navigate',{url:new URL(path,url).href});await until("document.readyState==='complete'");}
async function center(cell){await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');return evaluate(`(()=>{const r=document.querySelector('[data-cell="${cell}"]').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);}
async function mouse(type,p,pointerType='mouse'){await call('Input.dispatchMouseEvent',{type,pointerType,...p,button:type==='mouseMoved'?'none':'left',buttons:type==='mouseReleased'?0:1,clickCount:1});}
async function reset(){await evaluate("document.querySelector('#mode').value='two';document.querySelector('#mode').dispatchEvent(new Event('change'));document.querySelector('#new').click()");await until("document.querySelectorAll('#moves tr.best').length===2");}
async function gesture(a,b,{legal=true,cancel=false,touch=false,pen=false}={}){
 await evaluate('scrollTo(0,0)');
 const start=await center(a),end=await center(b);
 if(touch)await call('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{...start,id:1}]});else await mouse('mousePressed',start,pen?'pen':'mouse');
 if(touch)await call('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{...end,id:1}]});else await mouse('mouseMoved',end,pen?'pen':'mouse');
 await until("document.querySelector('.placement-preview')!==null");
 assert.equal(await evaluate("document.querySelector('.placement-preview')?.dataset.legal"),String(legal));
 // An analysis/language redraw must preserve the active captured gesture.
 await evaluate("document.querySelector('[data-language=en]').click()");
 assert.equal(await evaluate("document.querySelector('.placement-preview')?.dataset.legal"),String(legal));
 if(touch)await call('Input.dispatchTouchEvent',{type:cancel?'touchCancel':'touchEnd',touchPoints:[]});
 else if(cancel)await evaluate("document.querySelector('#board').dispatchEvent(new PointerEvent('pointercancel'))");
 else await mouse('mouseReleased',end,pen?'pen':'mouse');
 if(touch)assert.equal(await evaluate('scrollY'),0);
 assert.equal(await evaluate("document.querySelector('.placement-preview')"),null);
 assert.equal(await evaluate("document.querySelector('#sequence').value"),legal&&!cancel?'f3-f4':'');
}
try{
 const target=await call('Target.createTarget',{url:'about:blank'},null);session=(await call('Target.attachToTarget',{targetId:target.targetId,flatten:true},null)).sessionId;
 await call('Page.enable');await call('Runtime.enable');await call('Log.enable');
 await navigate('play/?lang=en');await until("document.querySelectorAll('#moves tr').length===20");
 assert.equal(await evaluate("document.querySelector('#analysis').checked"),false);
 assert.equal(await evaluate("document.querySelectorAll('#board .eval-label, #board rect[stroke=\"#d4a900\"]').length"),0);
 await navigate('play/?lang=en&analysis=1');await until("document.querySelectorAll('#moves tr').length===20 && document.querySelector('#position-value').textContent==='Exact +2'");
 assert.equal(await evaluate("document.documentElement.lang"),'en');
 assert.deepEqual(await evaluate("[...document.querySelector('#mode').options].map(o=>o.value)"),['black','white','two']);
 const hidden = "document.querySelectorAll('#board .eval-label, #board rect[stroke=\"#d4a900\"], #moves .best, #moves small').length===0 && [...document.querySelectorAll('#moves tr')].every(r=>r.children.length===1) && [...document.querySelectorAll('[data-analysis-only]')].every(e=>e.hidden) && document.querySelector('#position-value').textContent==='' && document.querySelector('#best-move').textContent==='' && document.querySelector('#source').textContent===''";
 await evaluate("document.querySelector('#analysis').click()");
 assert(await evaluate(hidden));
 assert.equal(await evaluate("localStorage.getItem('dosello-analysis')"),'0');
 await evaluate("document.querySelector('#mode').value='two';document.querySelector('#mode').dispatchEvent(new Event('change'))");
 await until("!document.querySelector('#hint').disabled");
 await evaluate("document.querySelector('#hint').click()");
 assert.equal(await evaluate("document.querySelectorAll('#board rect[stroke=\"#d4a900\"]').length"),1);
 assert.equal(await evaluate("document.querySelectorAll('#board .eval-label, #moves small').length"),0);
 assert.doesNotMatch(await evaluate("document.querySelector('#status').textContent"),/[a-h][1-8]|[+≈≤≥]/);
 await evaluate("document.querySelector('#moves button').click()");
 assert(await evaluate(hidden));
 await navigate('play/?lang=en');await until("document.querySelectorAll('#moves tr').length===20");
 assert(await evaluate(hidden));
 await navigate('play/?lang=en&analysis=1');await until("document.querySelectorAll('#moves tr.best').length===2");
 assert.equal(await evaluate("document.querySelector('#analysis').checked"),true);
 await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:'a',bubbles:true}))");assert(await evaluate(hidden));
 await evaluate("document.querySelector('#analysis-quick').click()");
 assert.equal(await evaluate("document.querySelector('#analysis').checked && localStorage.getItem('dosello-analysis')==='1'"),true);
 await navigate('play/?lang=en&analysis=0');await until("document.querySelectorAll('#moves tr').length===20");assert(await evaluate(hidden));
 await navigate('play/?lang=en');await until("document.querySelectorAll('#moves tr.best').length===2");

 // Review is explicit during play, works with Analysis off, and keeps book bounds.
 await evaluate("document.querySelector('#mode').value='two';document.querySelector('#mode').dispatchEvent(new Event('change'));document.querySelector('#sequence').value='a4-b4';document.querySelector('#apply').click()");
 assert.equal(await evaluate("document.querySelector('#review-panel').open"),false);
 await evaluate("document.querySelector('#analysis').click();window.reviewConfirms=0;window.confirm=()=>{window.reviewConfirms++;return true;};document.querySelector('#review').click()");
 await until("document.querySelector('#review-progress').textContent.includes('1/1')");
 assert.match(await evaluate("document.querySelector('#review-rows').textContent"),/≥ 4/);
 assert.equal(await evaluate("document.querySelectorAll('#review-rows .kind-badge').length"),3);
 assert.equal(await evaluate("document.querySelector('#review-rows tr').classList.contains('loss-red')"),true);
 await evaluate("document.querySelector('#review-rows button').click()");
 assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 assert.equal(await evaluate("document.querySelectorAll('#board rect[stroke=\"#d4a900\"]').length"),2);
 assert.equal(await evaluate("document.querySelectorAll('#board .eval-label').length"),0);
 await evaluate("document.querySelector('#review-end').click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'a4-b4');
 await evaluate("document.querySelector('#review-close').click();document.querySelector('#review').click()");
 assert.equal(await evaluate("window.reviewConfirms"),1);
 assert.match(await evaluate("document.querySelector('#review-progress').textContent"),/0 ms/);
 await evaluate("document.querySelector('#new').click()");assert.equal(await evaluate("document.querySelector('#review-panel').open"),false);
 assert.equal(await evaluate("document.querySelector('#time')"),null);
 // Phone end-of-game opens without confirmation; closing cancels, reopening finishes from cache.
 await call('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});
 await evaluate(`window.reviewStops=0;const BaseWorker=window.Worker;window.Worker=class extends BaseWorker{constructor(url,options){super(url,options);this.review=String(url).includes('review-worker');}terminate(){if(this.review)window.reviewStops++;super.terminate();}};document.querySelector('#sequence').value=${JSON.stringify(perfectLine.join(' '))};document.querySelector('#apply').click();`);
 assert.equal(await evaluate("document.querySelector('#review-panel').open"),true);
 assert.equal(await evaluate("window.reviewConfirms"),1);
 assert(await evaluate("document.documentElement.scrollWidth<=innerWidth && document.querySelector('#review-panel').getBoundingClientRect().width===document.querySelector('.board-area').getBoundingClientRect().width"));
 await evaluate("document.querySelector('#review-close').click()");assert.equal(await evaluate('window.reviewStops'),1);
 await evaluate("document.querySelector('#review').click()");
 await until("document.querySelector('#review-progress').textContent.includes('25/25')",300);
 assert.equal(await evaluate("document.querySelectorAll('#review-rows tr.loss-red:not(.estimated),#review-rows tr.loss-yellow:not(.estimated)').length"),0);
 await evaluate("document.querySelector('#review-close').click();document.querySelector('#review').click()");
 assert.match(await evaluate("document.querySelector('#review-progress').textContent"),/0 ms/);
 await evaluate("document.querySelector('#review-rows button').click();document.querySelector('#moves button').click()");
 assert.equal(await evaluate("document.querySelector('#review-panel').open"),false);
 assert.equal(await evaluate("document.querySelectorAll('#record button').length"),2);
 await call('Emulation.clearDeviceMetricsOverride');

 await navigate('play/?lang=en&analysis=1');await until("document.querySelectorAll('#moves tr.best').length===2");

 assert.match(await evaluate("[...document.querySelectorAll('#moves tr')].find(r=>r.querySelector('button').textContent==='f3-f4').textContent"),/\+2Exact/);
 assert.equal(await evaluate("document.querySelectorAll('#board rect[stroke=\"#d4a900\"]').length"),2);
 assert.deepEqual(await evaluate("[...document.querySelectorAll('#moves tr.best button')].map(x=>x.textContent).sort()"),['c5-c6','f3-f4']);
 await evaluate("document.querySelector('#mode').value='two';document.querySelector('#mode').dispatchEvent(new Event('change'));document.querySelector('#sequence').value='f3-f4 e6-f6 d7-e7';document.querySelector('#apply').click()");await until("document.querySelectorAll('#record button').length===4");
 await evaluate("document.querySelector('#undo').click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4 e6-f6');
 await evaluate("document.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}))");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4 e6-f6 d7-e7');
 await evaluate("document.querySelectorAll('#record button')[1].click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4');
 await evaluate("document.querySelector('[data-language=ja]').click()");assert.equal(await evaluate('document.documentElement.lang'),'ja');assert.equal(await evaluate('document.title'),'DOSELLO 解析ツール');
 await navigate('');await until("document.documentElement.lang==='ja'");assert.match(await evaluate("document.querySelector('#strength').textContent"),/397勝/);assert.match(await evaluate("document.querySelector('#computation').textContent"),/381勝2分17敗/);
 await navigate('?lang=en');await until("document.documentElement.lang==='en'");assert.match(await evaluate("document.querySelector('#strength').textContent"),/397 wins/);assert.match(await evaluate("document.querySelector('#computation').textContent"),/381 wins, 2 draws, 17 losses/);
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
 await navigate('play/?lang=en&analysis=1');await until("document.querySelectorAll('#moves tr.best').length===2");
 await evaluate("[...document.querySelectorAll('#moves button')].find(b=>b.textContent==='f3-f4').click()");await until("document.querySelectorAll('#record button').length>=3");
 await evaluate("document.querySelector('#undo').click()");assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 // Toggle twice during an AI turn: no worker restart/termination or game reset.
 assert(await evaluate(`(()=>{const NativeWorker=window.Worker;let starts=0,stops=0;window.Worker=class extends NativeWorker{constructor(...args){super(...args);starts++;}terminate(){stops++;super.terminate();}};document.querySelector('#mode').value='white';document.querySelector('#mode').dispatchEvent(new Event('change'));const before=[starts,stops,document.querySelector('#sequence').value];document.querySelector('#analysis').click();document.querySelector('#analysis-quick').click();const unchanged=JSON.stringify(before)===JSON.stringify([starts,stops,document.querySelector('#sequence').value]);window.Worker=NativeWorker;return unchanged;})()`));
 await until("document.querySelector('#sequence').value.length>0");
 await call('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:true});assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
 await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
 assert(await evaluate("document.querySelector('aside').getBoundingClientRect().top>=document.querySelector('.board-area').getBoundingClientRect().bottom"));
 assert(await evaluate("['settings','move-details','record-details'].every(id=>!document.getElementById(id).open)"));

 await reset();
 // Real mouse capture, legal/illegal moves, a tap and focused keyboard input.
 await gesture(21,29);await reset();await gesture(21,29,{pen:true});await reset();await gesture(0,1,{legal:false});
 const tap=await center(21);await mouse('mousePressed',tap);await mouse('mouseReleased',tap);
 assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 await evaluate("document.querySelector('[data-cell=\"21\"]').focus()");
 for(const key of ['Enter',' '])await call('Input.dispatchKeyEvent',{type:'keyDown',key});
 assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 await call('Input.dispatchKeyEvent',{type:'keyDown',key:'ArrowDown'});
 assert.equal(await evaluate("document.querySelector('#sequence').value"),'f3-f4');
 await reset();
 // Paused AI positions must still reject human placement for the AI's color.
 await evaluate("document.querySelector('#mode').value='white';document.querySelectorAll('#record button')[0].click()");
 await mouse('mousePressed',await center(21));await mouse('mouseMoved',await center(29));await mouse('mouseReleased',await center(29));
 assert.equal(await evaluate("document.querySelector('#sequence').value"),'');
 await reset();
 for(const [width,height] of [[360,740],[390,844],[430,932],[740,360],[844,390]]){
  await call('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:true});
  // Exercise collapsed and expanded content at every phone size.
  await evaluate("for(const id of ['settings','move-details','record-details'])document.getElementById(id).open=false");
  assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
  assert(await evaluate("(()=>{const r=document.querySelector('#board').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&Math.abs(r.width-r.height)<1})()"));
  assert(await evaluate("[...document.querySelectorAll('.actions button, .actions .analysis-toggle')].every(b=>b.getBoundingClientRect().height>=44 && b.getBoundingClientRect().width>=44)"));
  assert.equal(await evaluate("getComputedStyle(document.querySelector('#board')).touchAction"),'none');
  await evaluate("document.querySelector('#move-details').open=true;document.querySelector('#record-details').open=true;document.querySelector('#settings').open=true");
  assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
  if(width===390){
   await evaluate("for(const id of ['settings','move-details','record-details'])document.getElementById(id).open=false");
   const shot=await call('Page.captureScreenshot',{format:'png'});writeFileSync(join(tmpdir(),'dosello-mobile.png'),Buffer.from(shot.data,'base64'));
   await call('Emulation.setTouchEmulationEnabled',{enabled:true});
   await gesture(21,29,{touch:true,cancel:true});
   await gesture(21,29,{touch:true});await reset();
   await call('Emulation.setTouchEmulationEnabled',{enabled:false});
  }
 }
 await call('Emulation.clearDeviceMetricsOverride');
 await call('Emulation.setScriptExecutionDisabled',{value:true});await navigate('?lang=en');assert.equal(await evaluate('document.documentElement.lang'),'en');assert.match(await evaluate("document.querySelector('#strength').textContent"),/397 wins/);assert.match(await evaluate("document.querySelector('#computation').textContent"),/381 wins, 2 draws, 17 losses/);
 assert.deepEqual(errors.filter(e=>!e.includes('favicon')),[]);
 console.log('PASS browser: progressive review, bound loss, phone auto-open, cancellation, cached reopen, retry, fixed AI time, analysis visibility, single-move hint, preference/URL/shortcut, uninterrupted AI, CSP/WASM/Worker, 20 moves, book +2, best outlines, sequence, undo/redo, record jump, both AI colors, language persistence/override, translated PV, mouse/touch/pen drag and cancellation, illegal preview, keyboard placement, AI gating, portrait and landscape layouts');
}finally{clearTimeout(deadline);await call('Browser.close',{},null).catch(()=>{});chrome.kill();await new Promise(r=>setTimeout(r,500));rmSync(profile,{recursive:true,force:true});}
