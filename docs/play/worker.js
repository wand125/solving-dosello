import init from './wasm/dosello.js';
const ready=(async()=>{const ai=await init();let bookError=null;try{await ai.loadBook(new URL('./wasm/opening-book.bin',import.meta.url));}catch(e){bookError=e.message;}return {ai,bookError};})();
self.onmessage=async({data})=>{
 try{
  const {ai,bookError}=await ready;
  const book=ai.getBook(data.position);
  self.postMessage({id:data.id,kind:'book',book,bookError});
  const result=ai.analyze(data.position,{timeMs:data.timeMs,ttMb:32,bestOnly:data.bestOnly});
  self.postMessage({id:data.id,kind:'result',book,result});
 }catch(e){self.postMessage({id:data.id,kind:'error',error:e.message});}
};
