import init from './wasm/dosello.js';
import {fromOriginal,toOriginal,legalMoves,applyMove,formatMove} from './engine/rules.js';
import {provenBest} from './analysis.js';
const ready=(async()=>{const ai=await init();let bookError=null;try{await ai.loadBook(new URL('./wasm/opening-book.bin',import.meta.url));}catch(e){bookError=e.message;}return {ai,bookError};})();
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
self.onmessage=async({data})=>{
 try{
  const {ai,bookError}=await ready;
  const book=ai.getBook(data.position);
  if(book?.moves){
   const root=fromOriginal(data.position);
   for(const m of book.moves)if(m.exact)m.pv=provenLine(ai,root,m.move);
  }
  self.postMessage({id:data.id,kind:'book',book,bookError});
  const result=ai.analyze(data.position,{timeMs:data.timeMs,ttMb:32,bestOnly:data.bestOnly});
  self.postMessage({id:data.id,kind:'result',book,result});
 }catch(e){self.postMessage({id:data.id,kind:'error',error:e.message});}
};
