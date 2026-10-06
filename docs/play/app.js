import {initialState,legalMoves,applyMove,pass,isGameOver,score,toOriginal,formatMove} from './engine/rules.js';
import {renderBoard} from './view.js';
import {valueLabel,mergeValues,bestMoves} from './analysis.js';
const $=id=>document.getElementById(id);
let state=initialState(),history=[],selected=null,analysis=[],best=[],worker=null,generation=0,busy=false,ready=false,hinted=false;
const human=()=>$('mode').value==='two'||state.turn===($('mode').value==='black'?1:-1);
function cancel(){generation++;worker?.terminate();worker=null;busy=false;}
function draw(){
 const ms=legalMoves(state),s=score(state),labels=new Map(analysis.map(m=>[m.move,valueLabel(m)]));
 $('black').textContent='● '+s.black;$('white').textContent='○ '+s.white;
 $('turn').textContent=isGameOver(state)?'終局':(state.turn===1?'黒':'白')+'の手番';
 const active=human()&&!isGameOver(state);
 renderBoard($('board'),state,{moves:ms,values:labels,best:$('overlay').checked||hinted?best:[],selected,overlay:$('overlay').checked,onCell:active?selectCell:null,onMove:active?play:null});
 $('count').textContent=String(ms.length);$('moves').replaceChildren();
 for(const m of [...ms].sort((a,b)=>(analysis.find(v=>v.move===formatMove(b))?.value??-100)-(analysis.find(v=>v.move===formatMove(a))?.value??-100))){
  const name=formatMove(m),button=document.createElement('button');button.disabled=!active;
  const coord=document.createElement('span'),value=document.createElement('span');coord.textContent=name;value.textContent=labels.get(name)??'…';button.append(coord,value);
  if(best.includes(name))button.className='best';button.onclick=()=>play(m);$('moves').append(button);
 }
 $('undo').disabled=!history.length;$('hint').disabled=!active||!ready;
}
function selectCell(i){
 if(state.board[i])return;
 if(selected===null){selected=i;draw();return;}
 const move=legalMoves(state).find(m=>m.cells.includes(selected)&&m.cells.includes(i));
 if(move)play(move);else{selected=i;draw();$('status').textContent='隣り合う合法な2マスを選んでください。';}
}
function play(m){history.push(state);state=applyMove(state,m);refresh();}
function refresh(){
 cancel();selected=null;analysis=[];best=[];hinted=false;ready=false;
 let passed='';if(!legalMoves(state).length&&!isGameOver(state)){passed=(state.turn===1?'黒':'白')+'はパス。';state=pass(state);}
 if(isGameOver(state)){const s=score(state);$('status').textContent=`終局：黒 ${s.black} 対 白 ${s.white}。`+(s.difference===0?'引き分け。':s.difference>0?'黒の勝ち。':'白の勝ち。');draw();return;}
 $('status').textContent=passed+(human()?'合法手を選べます。評価を計算中…':'AIが考えています…');
 draw();busy=true;const id=generation;worker=new Worker(new URL('./worker.js',import.meta.url),{type:'module'});
 worker.onerror=()=>{if(id!==generation)return;busy=false;$('status').textContent='エンジンを読み込めませんでした。新しい対局で再試行するか、2人対局をご利用ください。';draw();};
 worker.onmessage=({data})=>{
  if(id!==generation||data.id!==id)return;
  if(data.kind==='error'){busy=false;$('status').textContent='解析エラー：'+data.error;draw();return;}
  ready=true;analysis=mergeValues(data.book,data.result);best=bestMoves(analysis);draw();
  if(data.kind==='book'){if(data.bookError)$('source').textContent='定石を読み込めないため、WASM探索を使用中';return;}
  busy=false;$('source').textContent=data.result.source==='book'?'定石の証明値を使用':'WASM探索・'+Math.round(data.result.elapsedMs??0)+' ms';
  if(!human()){
   const name=data.result.bestMove??best[0],move=legalMoves(state).find(m=>formatMove(m)===name);
   if(move){play(move);return;}$('status').textContent='AIが合法手を返しませんでした。新しい対局で再試行してください。';
  }else $('status').textContent=passed+'あなたの手番です。黄色は最善候補。';
 };
 worker.postMessage({id,position:toOriginal(state),timeMs:Number($('time').value),bestOnly:!human()});
}
$('new').onclick=()=>{state=initialState();history=[];refresh();};$('mode').onchange=$('new').onclick;
$('time').onchange=()=>refresh();$('overlay').onchange=()=>draw();
$('hint').onclick=()=>{hinted=true;draw();$('status').textContent=best.length?'候補：'+best.join(' / '):'解析結果を待っています。';};
$('undo').onclick=()=>{if(!history.length)return;cancel();state=history.pop();if($('mode').value!=='two')while(history.length&&!human())state=history.pop();refresh();};
refresh();
