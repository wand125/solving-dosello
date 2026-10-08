import init from './wasm/dosello.js';
import {ReviewCache,reviewGame} from './review.js';
self.onmessage=async({data})=>{
 try{
  const ai=await init();try{await ai.loadEval3(new URL('./wasm/eval3-r2.bin',import.meta.url));}catch{/* Fall back to the previous evaluator. */}try{await ai.loadBook(new URL('./wasm/display-book.bin',import.meta.url));}catch{self.postMessage({kind:'bookError'});}
  const cache=new ReviewCache(data.cache),start=performance.now();
  reviewGame(ai,data.states,data.moves,{...data.options,cache,
   onStage:(index,stage)=>self.postMessage({kind:'stage',index,stage}),
   onRow:(index,row)=>self.postMessage({kind:'row',index,row})});
  self.postMessage({kind:'done',elapsedMs:performance.now()-start});
 }catch(e){self.postMessage({kind:'error',error:e.message});}
};
