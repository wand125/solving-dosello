import {applyMove,parseMove,toOriginal,score,isGameOver} from './engine/rules.js';
import {valueKind,provenBest} from './analysis.js';

// Only proven intervals participate in guarantees. An estimate is a point guess,
// never a zero-width proof interval.
export function reviewValue(v){
 if(!v||!Number.isFinite(v.value))return null;
 const kind=v.kind??valueKind(v);
 return {kind,value:v.value,lower:kind==='exact'?v.value:kind==='bound'?(v.lower??-64):-64,upper:kind==='exact'?v.value:kind==='bound'?(v.upper??64):64};
}
export function lossBetween(best,played){
 if(!best||!played)return null;
 if(best.kind==='estimate'||played.kind==='estimate')return {kind:'estimate',value:Math.max(0,best.value-played.value),lower:0,upper:128};
 const lower=Math.max(0,best.lower-played.upper),upper=Math.max(0,best.upper-played.lower);
 return {kind:lower===upper?'exact':'bound',value:lower,lower,upper,upperOpen:best.kind==='bound'&&best.upper===64||played.kind==='bound'&&played.lower===-64};
}
export function lossLabel(loss){
 if(!loss)return '…';
 if(loss.kind==='estimate')return '≈ '+Math.round(loss.value);
 if(loss.kind==='exact')return String(loss.value);
 if(loss.lower===0)return '≤ '+loss.upper;
 if(loss.upperOpen)return '≥ '+loss.lower;
 return `[${loss.lower}, ${loss.upper}]`;
}
export function lossColor(loss){
 if(!loss)return 'pending';
 const n=loss.kind==='estimate'?loss.value:loss.lower;
 return (loss.kind==='estimate'?'estimated ':'')+(n>=4?'loss-red':n>0?'loss-yellow':loss.upper===0?'loss-zero':'loss-neutral');
}
// Disjoint possible W/D/L categories prove a change even for win → [loss, draw].
function outcomes(v){return !v||v.kind==='estimate'?null:[v.lower<0,v.lower<=0&&v.upper>=0,v.upper>0];}
export function firstDecisive(rows){return rows.findIndex(r=>{const a=outcomes(r?.best),b=outcomes(r?.played);return a!==null&&b!==null&&!a.some((possible,i)=>possible&&b[i]);});}
export function reviewSummary(rows){
 const sides={1:{total:0,mistakes:0},'-1':{total:0,mistakes:0}};
 for(const r of rows){if(!r?.loss||r.loss.kind==='estimate')continue;const side=sides[r.side];side.total+=r.loss.lower;if(r.loss.lower>0)side.mistakes++;}
 return {sides,decisive:firstDecisive(rows)};
}
const notation=i=>'abcdefgh'[i%8]+(1+(i>>3));
function moveThrough(move,map){return move.split('-').map(c=>map[(+c[1]-1)*8+c.charCodeAt(0)-97]).sort((a,b)=>a-b).map(notation).join('-');}
// D4 without color exchange; pairing and side are essential parts of the key.
export function canonicalPosition(s){
 let result;
 for(let symmetry=0;symmetry<8;symmetry++){
  const map=Array.from({length:64},(_,i)=>{let x=i%8,y=i>>3;if(symmetry&4)x=7-x;for(let r=0;r<(symmetry&3);r++)[x,y]=[7-y,x];return y*8+x;});
  const cells=Array(64);for(let i=0;i<64;i++)cells[map[i]]=s.board[i]?`${s.board[i]===1?'b':'w'}${String(map[s.pair[i]]).padStart(2,'0')}`:'___';
  const key=s.turn+':'+cells.join('');
  if(!result||key<result.key){const inverse=[];map.forEach((v,i)=>inverse[v]=i);result={key,map,inverse};}
 }
 return result;
}
export class ReviewCache {
 constructor(entries=[]){this.positions=new Map(entries.map(([k,v])=>[k,new Map(v)]));}
 get(s,move){const c=canonicalPosition(s),r=this.positions.get(c.key)?.get(moveThrough(move,c.map));return r?{...structuredClone(r),bestMoves:r.bestMoves.map(m=>moveThrough(m,c.inverse))}:null;}
 set(s,move,row){const c=canonicalPosition(s);if(!this.positions.has(c.key))this.positions.set(c.key,new Map());this.positions.get(c.key).set(moveThrough(move,c.map),{...structuredClone(row),bestMoves:row.bestMoves.map(m=>moveThrough(m,c.map))});}
 entries(){return [...this.positions].map(([k,v])=>[k,[...v]]);}
}
function negate(v){return v&&{...v,value:-v.value,lower:-v.upper,upper:-v.lower};}
function positionBook(ai,s){
 const b=ai.getBook(toOriginal(s));
 if(!b)return null;
 const best=reviewValue(b),bestMoves=b.exact?b.moves.filter(m=>m.exact&&m.value===b.value).map(m=>m.move):provenBest(b.moves);
 return {best,bestMoves,moves:b.moves};
}
function terminalValue(s){return isGameOver(s)?reviewValue({exact:true,value:score(s).difference*s.turn}):null;}
function proven(v){return v&&v.kind!=='estimate';}
function finished(r){return proven(r.best)&&proven(r.played)&&r.bestMoves.length>0&&r.bestMovesProven;}
function resultValue(r){
 if(r.exact&&Number.isFinite(r.value))return reviewValue(r);
 if(r.terminal)return reviewValue({value:r.value,exact:true});
 // bestOnly contains one chosen move, whose exact flag certifies the root.
 const m=r.moves?.find(m=>m.move===r.bestMove)??r.moves?.[0];
 return m?reviewValue(m):Number.isFinite(r.value)?reviewValue({value:r.value}):null;
}
export const reviewDefaults=phone=>({exactEmpties:phone?26:30,estimateMs:phone?500:1000,exactMs:phone?1000:2000});

// Three passes ensure every available book result is delivered before any solve.
// Synchronous WASM runs only in the review Worker (or offline Node tests).
export function reviewGame(ai,states,moves,{cache=new ReviewCache(),onRow=()=>{},onStage=()=>{},...options}={}){
 const config={...reviewDefaults(false),...options},rows=[];
 const publish=(i,r)=>{r.loss=lossBetween(r.best,r.played);rows[i]=r;cache.set(states[i],moves[i],r);onRow(i,r);};
 for(let i=0;i<moves.length;i++){
  const s=states[i],cached=cache.get(s,moves[i]);
  if(cached?.done){publish(i,cached);continue;}
  const b=positionBook(ai,s),child=applyMove(s,parseMove(s,moves[i]));
  const direct=reviewValue(b?.moves.find(m=>m.move===moves[i])),reply=negate(terminalValue(child)??positionBook(ai,child)?.best);
  // A heuristic in the parent book must not hide an exact child proof.
  const played=direct?.kind==='exact'?direct:reply?.kind==='exact'?reply:proven(direct)?direct:reply??direct;
  const r={best:b?.best??null,played,bestMoves:b?.bestMoves??[],bestMovesProven:!!b?.bestMoves.length,side:s.turn,source:'book',done:false,...cached};
  r.done=finished(r);publish(i,r);
 }
 const solve=(i,exact)=>{
  const r=rows[i],s=states[i],child=applyMove(s,parseMove(s,moves[i]));
  onStage(i,exact?'solved':'search');
  const start=performance.now(),budget=exact?config.exactMs:config.estimateMs;
  const run=(position,valueOnly,timeMs)=>ai.analyze(toOriginal(position),{timeMs:Math.floor(timeMs),ttMb:32,bestOnly:true,exactIfPossible:false,reviewSolve:exact,valueOnly});
  if(!proven(r.best)||!r.bestMoves.length){
   const a=run(s,false,proven(r.played)?budget:budget/2),v=resultValue(a);
   if(v&&(!proven(r.best)||v.kind==='exact'))r.best=v;
   if(a.bestMove&&(a.exact||!exact)){r.bestMoves=[a.bestMove];r.bestMovesProven=v?.kind==='exact';}
  }
  // A proven best placement already has the exact root value.
  if(r.best?.kind==='exact'&&r.bestMovesProven&&r.bestMoves.includes(moves[i]))r.played={...r.best};
  if(!proven(r.played)){
   const a=run(child,true,Math.max(0,budget-(performance.now()-start))),v=resultValue(a);
   if(v)r.played=negate(v);
  }
  r.source=exact?'solved':'search';r.done=finished(r)||!exact;publish(i,r);
 };
 for(let i=0;i<moves.length;i++)if(!rows[i].done&&score(states[i]).empty<=config.exactEmpties)solve(i,true);
 for(let i=0;i<moves.length;i++)if(!rows[i].done)solve(i,false);
 return rows;
}
