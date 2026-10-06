import {ReviewCache,reviewDefaults,lossLabel,lossColor,reviewSummary} from './review.js';
import {valueLabel} from './analysis.js';
import {isGameOver} from './engine/rules.js';
const $=id=>document.getElementById(id);
export function reviewPanel({game,jump,pause,redraw,t,phone,analysisEnabled}){
 const cache=new ReviewCache();let worker=null,rows=[],stages=[],snapshot=null,confirmed=false,autoKey=null,elapsed=null,error=false,bookError=false;
 const active=()=>$('review-panel').open;
 const stop=()=>{worker?.terminate();worker=null;};
 function close(){stop();$('review-panel').open=false;redraw();}
 function render(){
  $('review-rows').replaceChildren();
  if(!snapshot)return;
  const g=game();
  const valueCell=(tr,v,loss=false)=>{
   const cell=document.createElement('td');cell.textContent=v?(loss?lossLabel(v):valueLabel({...v,exact:v.kind==='exact'},'').trim()):'…';
   if(v){const badge=document.createElement('small');badge.className='kind-badge '+v.kind;badge.textContent=t(v.kind);cell.append(badge);}tr.append(cell);
  };
  snapshot.moves.forEach((move,i)=>{
   const r=rows[i],tr=document.createElement('tr');tr.className=lossColor(r?.loss);
   if(g.mode!=='two'&&snapshot.states[i].turn===(g.mode==='black'?1:-1))tr.classList.add('human-move');
   tr.setAttribute('aria-selected',String(g.cursor===i));
   const button=document.createElement('button');button.textContent=`${i+1}. ${move}`;button.onclick=()=>jump(i);
   const td=document.createElement('td');td.append(button);tr.append(td);
   const bestKind=r?.bestMovesProven?t('best'):t('recommended');
   for(const text of [t(snapshot.states[i].turn===1?'black':'white'),r?.bestMoves.length?`${r.bestMoves.join(' / ')} · ${bestKind}`:'…']){
    const cell=document.createElement('td');cell.textContent=text;tr.append(cell);
   }
   valueCell(tr,r?.best);valueCell(tr,r?.played);valueCell(tr,r?.loss,true);
   const status=document.createElement('td');status.textContent=r?.done?t(r.source):t(stages[i]??'loading');tr.append(status);
   tr.onclick=e=>{if(!e.target.closest('button'))jump(i);};$('review-rows').append(tr);
  });
  const summary=reviewSummary(rows),parts=[1,-1].map(side=>`${t(side===1?'black':'white')}: ≥ ${summary.sides[side].total} ${t('discs')}, ${summary.sides[side].mistakes} ${t('mistakes')}`);
  parts.push(`${t('decisive')}: ${summary.decisive<0?t('noneProven'):summary.decisive+1}`);
  $('review-summary').textContent=parts.join(' · ');
  $('review-progress').textContent=error?t('error'):`${rows.filter(r=>r?.done).length}/${snapshot.moves.length} · ${elapsed===null?t(worker?'loading':'reviewStopped'):`${t('idle')} · ${Math.round(elapsed)} ms`}${bookError?' · '+t('bookError'):''}`;
 }
 function open(automatic=false){
  if(active())return;
  const g=game();
  if(!automatic&&!analysisEnabled()&&!isGameOver(g.states.at(-1))&&!confirmed){if(!window.confirm(t('reviewConfirm')))return;confirmed=true;}
  pause();snapshot={states:g.states.slice(),moves:g.moves.slice()};rows=snapshot.moves.map((m,i)=>cache.get(snapshot.states[i],m));stages=[];elapsed=null;error=false;bookError=false;$('review-panel').open=true;
  if(rows.every(r=>r?.done)){elapsed=0;render();redraw();return;}
  worker=new Worker(new URL('./review-worker.js',import.meta.url),{type:'module'});const current=worker;
  worker.onmessage=({data})=>{
   if(worker!==current)return;
   if(data.kind==='row'){rows[data.index]=data.row;cache.set(snapshot.states[data.index],snapshot.moves[data.index],data.row);}
   if(data.kind==='bookError')bookError=true;
   if(data.kind==='stage')stages[data.index]=data.stage;
   if(data.kind==='done'){elapsed=data.elapsedMs;stop();}
   if(data.kind==='error'){error=true;stop();}
   render();redraw();
  };
  worker.onerror=()=>{if(worker!==current)return;error=true;stop();render();};
  const options=reviewDefaults(phone());
  const configured=Number(new URLSearchParams(location.search).get('reviewEmpties'));
  if(new URLSearchParams(location.search).has('reviewEmpties')&&Number.isInteger(configured)&&configured>=0&&configured<=56)options.exactEmpties=configured;
  worker.postMessage({...snapshot,options,cache:cache.entries()});render();redraw();
 }
 $('review').onclick=()=>open();
 $('review-panel').querySelector('summary').onclick=e=>{e.preventDefault();if(active())close();else open();};
 $('review-panel').addEventListener('toggle',()=>{if(!active()){stop();redraw();}});
 $('review-close').onclick=close;
 $('review-end').onclick=()=>jump(game().moves.length);
 return {active,close,render,
  outlines:()=>active()?rows[game().cursor]?.bestMoves??[]:null,
  reset:()=>{close();rows=[];snapshot=null;autoKey=null;render();},
  auto:()=>{const g=game(),key=g.moves.join(' ');if(g.cursor===g.moves.length&&isGameOver(g.states.at(-1))&&autoKey!==key){autoKey=key;open(true);}}
 };
}
