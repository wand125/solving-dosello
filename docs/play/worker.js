import init from './wasm/dosello.js';
import {loadToolBook} from './wasm/tool-book.js';
import {fromOriginal,toOriginal,legalMoves,applyMove,formatMove} from './engine/rules.js';
import {provenBest} from './analysis.js';
const ready=(async()=>{
 const ai=await init();
 let bookError=null,tool=null;
 await Promise.all([
  ai.loadEval3(new URL('./wasm/eval3-r2.bin',import.meta.url)).catch(()=>{}),
  ai.loadBook(new URL('./wasm/display-book.bin',import.meta.url)).catch(e=>{bookError=e.message;}),
  loadToolBook(new URL('./wasm/tool-book.bin',import.meta.url)).then(b=>{tool=b;}).catch(()=>{}),
 ]);
 return {ai,bookError,tool};
})();
// A proven move's PV must follow proven best replies, not a heuristic search line:
// walk the book while each position has a proven best move.
function provenLine(ai,state,move,limit=16){
 const line=[move];
 for(let i=0;i<limit;i++){
  const m=legalMoves(state).find(x=>formatMove(x)===line[line.length-1]);
  if(!m)break;
  state=applyMove(state,m);
  const best=provenBest(ai.getBook(toOriginal(state))?.moves??[]);
  if(!best.length)break;
  line.push(best[0]);
 }
 return line;
}
// Pondering: while the player thinks, search their position in short slices on
// a TT kept inside the WASM instance. Each slice yields to the event loop, so a
// move request waits at most one slice and then reuses the entries.
const PONDER_SLICE_MS=200;
let ponderId=0;
async function ponder(position,limitMs=60000){
 const id=++ponderId,{ai}=await ready,start=performance.now();
 while(id===ponderId&&performance.now()-start<limitMs){
  let r;
  try{r=ai.analyze(position,{timeMs:PONDER_SLICE_MS,ttMb:32,bestOnly:true,useBook:false,keepTT:true});}catch{return;}
  if(r.terminal||r.pass||r.moves?.[0]?.exact)return;
  await new Promise(resolve=>setTimeout(resolve,0));
 }
}
self.onmessage=async({data})=>{
 if(data.kind==='ponder'){ponder(data.position,data.limitMs);return;}
 ponderId++;
 if(data.kind==='stop')return;
 try{
  // A queued request loses the time it waited behind the last ponder slice.
  const timeMs=Math.max(0,data.timeMs-(data.sentAt?Math.max(0,Date.now()-data.sentAt):0));
  const {ai,bookError,tool}=await ready;
  const toolResult=tool?.lookup(data.position)??null;
  const book=ai.getBook(data.position);
  if(book?.moves){
   const root=fromOriginal(data.position);
   for(const m of book.moves)if(m.exact)m.pv=provenLine(ai,root,m.move);
  }
  self.postMessage({id:data.id,kind:'book',book,bookError,tool:toolResult});
  const result=data.bestOnly&&toolResult?toolResult:ai.analyze(data.position,{timeMs,ttMb:32,bestOnly:data.bestOnly,useBook:!data.bestOnly,keepTT:!!data.keepTT});
  self.postMessage({id:data.id,kind:'result',book,result,bookError,tool:toolResult});
 }catch(e){self.postMessage({id:data.id,kind:'error',error:e.message});}
};
