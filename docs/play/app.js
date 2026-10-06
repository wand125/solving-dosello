import {initialState,legalMoves,applyMove,isGameOver,score,toOriginal,formatMove} from './engine/rules.js';
import {renderBoard} from './view.js';
import {valueLabel,valueKind,mergeValues,bestMoves,provenBest} from './analysis.js';
import {normalize,parseSequence} from './record.js';
import {dictionaries} from './i18n.js';
import {languageController} from '../language.js';
const $=id=>document.getElementById(id);
let states=[initialState()],record=[],cursor=0,selected=null,analysis=[],book=null,result=null,best=[],worker=null,generation=0,busy=false,hinted=false,paused=false,descending=true,lang='en',message='loading',detail='';
const state=()=>states[cursor],t=k=>dictionaries[lang][k];
const human=()=>['two','analysis'].includes($('mode').value)||state().turn===($('mode').value==='black'?1:-1);
function status(key,extra=''){message=key;detail=extra;$('status').textContent=t(key)+(extra?' '+extra:'');}
function cancel(){generation++;worker?.terminate();worker=null;busy=false;}
function draw(){
 const s=state(),ms=legalMoves(s),counts=score(s),labels=new Map(analysis.filter(m=>Number.isFinite(m.value)).map(m=>[m.move,valueLabel(m,'').trim()]));
 $('black').textContent='● '+counts.black;$('white').textContent='○ '+counts.white;
 $('turn').textContent=isGameOver(s)?t('over'):(s.turn===1?t('black'):t('white'))+' '+t('turn');
 const active=(human()||paused)&&!isGameOver(s);
 renderBoard($('board'),s,{moves:ms,values:labels,best,selected,overlay:$('overlay').checked||hinted,lastMove:record[cursor-1],onCell:active?selectCell:null,onMove:active?play:null});
 $('board').setAttribute('aria-label',t('board'));$('count').textContent=String(ms.length);$('moves').replaceChildren();
 const proven=provenBest(analysis),values=new Map(analysis.map(m=>[m.move,m]));
 const sorted=[...ms].sort((a,b)=>{const av=values.get(formatMove(a))?.value,bv=values.get(formatMove(b))?.value;return av===undefined?bv===undefined?0:1:bv===undefined?-1:(descending?bv-av:av-bv);});
 for(const m of sorted){
  const name=formatMove(m),entry=values.get(name),v=Number.isFinite(entry?.value)?entry:null,row=document.createElement('tr'),button=document.createElement('button');button.textContent=name;button.disabled=!active;button.onclick=()=>play(m);
  const cell=document.createElement('td');cell.append(button);row.append(cell);
  if(best.includes(name)){row.className='best';const tag=document.createElement('small');tag.textContent=t(proven.length?'best':'recommended');cell.append(tag);}
  for(const text of [v?valueLabel(v,'').trim():'…',v?t(valueKind(v)):'…',v?.depth??'—',v?.pv?.join(' ')||'—']){const td=document.createElement('td');td.textContent=text;td.title=String(text);if(v)td.className=valueKind(v);row.append(td);}
  $('moves').append(row);
 }
 $('position-value').textContent=book?valueLabel(book,t('exact')):isGameOver(s)?valueLabel({exact:true,value:counts.difference*s.turn},t('exact')):result?.value!=null?valueLabel({value:result.value,exact:result.moves?.length>0&&result.moves.every(m=>m.exact)},t('exact')):t('unknown');
 $('record').replaceChildren();
 for(let i=0;i<=record.length;i++){const b=document.createElement('button');b.textContent=i?`${i}. ${record[i-1]}`:t('initial');b.setAttribute('aria-current',String(i===cursor));b.onclick=()=>jump(i);$('record').append(b);}
 $('undo').disabled=cursor===0;$('redo').disabled=cursor===record.length;$('hint').disabled=!analysis.length;
 const source=isGameOver(s)?'solved':book?.exact?'book':result?.moves?.length&&result.moves.every(m=>m.exact)?'solved':'search';
 $('source').textContent=`${t(busy?'loading':'idle')} · ${t(source)} · ${result?.nodes??0} ${t('nodes')} · ${t('depth')} ${Math.max(0,...analysis.map(m=>m.depth??0))} · ${Math.round(result?.elapsedMs??0)} ms`;
 if(isGameOver(s)){message='over';detail=`${counts.black}–${counts.white} · ${counts.difference===0?t('draw'):t(counts.difference>0?'black':'white')+' '+t('wins')}`;}
 status(message,detail);
}
function selectCell(i){if(state().board[i])return;if(selected===null){selected=i;draw();return;}const m=legalMoves(state()).find(m=>m.cells.includes(selected)&&m.cells.includes(i));if(m)play(m);else{selected=i;status('select');draw();}}
function syncSequence(){$('sequence').value=record.slice(0,cursor).join(' ');}
function play(m){states=states.slice(0,cursor+1);record=record.slice(0,cursor);record.push(formatMove(m));states.push(normalize(applyMove(state(),m)));cursor++;paused=false;syncSequence();refresh();}
function jump(i){cursor=i;paused=true;syncSequence();refresh();}
function refresh(){
 cancel();selected=null;analysis=[];book=null;result=null;best=[];hinted=false;
 if(isGameOver(state())){const s=score(state());status('over',`${s.black}–${s.white} · ${s.difference===0?t('draw'):t(s.difference>0?'black':'white')+' '+t('wins')}`);draw();return;}
 status(paused?'paused':human()?'loading':'thinking');busy=true;draw();const id=generation;
 worker=new Worker(new URL('./worker.js',import.meta.url),{type:'module'});
 worker.onerror=()=>{if(id!==generation)return;busy=false;status('error');draw();};
 worker.onmessage=({data})=>{
  if(id!==generation||data.id!==id)return;
  if(data.kind==='error'){busy=false;status('error');draw();return;}
  book=data.book;result=data.result??null;const merged=new Map(mergeValues(book,result).map(m=>[m.move,m]));analysis=legalMoves(state()).map(m=>merged.get(formatMove(m))??{move:formatMove(m)});best=bestMoves(analysis);
  // A proven best move from the book is played at once; search only decides outside proven positions.
  const proven=provenBest(analysis);
  if(data.kind==='book'){
   if(!human()&&!paused&&proven.length){const m=legalMoves(state()).find(m=>formatMove(m)===proven[0]);if(m){cancel();play(m);return;}}
   if(data.bookError)status('bookError');draw();return;}
  busy=false;
  if(!human()&&!paused){const name=proven[0]??result.bestMove??best[0],m=legalMoves(state()).find(m=>formatMove(m)===name);if(m){play(m);return;}status('error');}
  else status(paused?'paused':data.bookError?'bookError':'idle');draw();
 };
 worker.postMessage({id,position:toOriginal(state()),timeMs:Number($('time').value),bestOnly:!human()&&!paused});
}
$('new').onclick=()=>{states=[initialState()];record=[];cursor=0;paused=false;syncSequence();refresh();};
$('mode').onchange=()=>{paused=false;refresh();};$('time').onchange=refresh;$('overlay').onchange=draw;
$('hint').onclick=()=>{hinted=true;status(best.length?'candidate':'waiting',best.join(' / '));draw();};
$('undo').onclick=()=>{if(!cursor)return;let i=cursor-1;if(!['two','analysis'].includes($('mode').value))while(i>0&&states[i].turn!==($('mode').value==='black'?1:-1))i--;jump(i);};
$('redo').onclick=()=>{if(cursor<record.length)jump(cursor+1);};
$('sort').onclick=()=>{descending=!descending;$('value-heading').setAttribute('aria-sort',descending?'descending':'ascending');$('sort-arrow').textContent=descending?'↓':'↑';draw();};
$('apply').onclick=()=>{try{const parsed=parseSequence($('sequence').value);states=parsed.states;record=parsed.moves;cursor=record.length;paused=true;syncSequence();refresh();}catch(e){status('invalid',e.message);}};
$('copy').onclick=async()=>{syncSequence();try{await navigator.clipboard.writeText($('sequence').value);status('copied');}catch{$('sequence').select();status('copyFallback');}};
 document.addEventListener('keydown',e=>{if(e.ctrlKey||e.metaKey||e.altKey||/INPUT|TEXTAREA|SELECT/.test(e.target.tagName)||e.target.isContentEditable)return;const id={n:'new',u:'undo',ArrowLeft:'undo',ArrowRight:'redo',h:'hint',e:'overlay'}[e.key.length===1?e.key.toLowerCase():e.key];if(!id)return;e.preventDefault();$(id).click();});
languageController(value=>{lang=value;document.title=t('title');document.querySelector('meta[name=description]').content=t('description');for(const el of document.querySelectorAll('[data-i18n]'))el.textContent=t(el.dataset.i18n);draw();});
refresh();
