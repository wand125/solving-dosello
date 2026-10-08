import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import init,{decodeBook} from '../docs/play/wasm/dosello.js';
import {decodeToolBook,loadToolBook} from '../docs/play/wasm/tool-book.js';
import {cell} from '../docs/play/wasm/book-symmetry.js';
import {initialState,toOriginal,fromOriginal,legalMoves,formatMove} from '../engine/rules.js';
const read=p=>readFileSync(new URL(p,import.meta.url));
const toolBytes=read('../docs/play/wasm/tool-book.bin');
const displayBytes=read('../docs/play/wasm/display-book.bin');
const root=toOriginal(initialState());
function toolRow(index){
 const row=toolBytes.subarray(12+index*36,48+index*36);
 const b=new Uint8Array(12+34);b.set(new TextEncoder().encode('DSBOOK02'));new DataView(b.buffer).setUint32(8,1,true);b.set(row.subarray(0,33),12);
 return {position:decodeBook(b).entries[0].position,cells:[row[33],row[34]],value:row.readInt8(35)};
}
function rotate(position,t){
 const s=fromOriginal(position),q={board:new Int8Array(64),pair:new Int8Array(64).fill(-1),turn:s.turn};
 for(let a=0;a<64;a++)if(s.board[a]){q.board[cell(a,t)]=s.board[a];q.pair[cell(a,t)]=cell(s.pair[a],t);}
 return toOriginal(q);
}
test('TOOL returns the proven root and sample moves through all eight symmetries',()=>{
 const book=decodeToolBook(toolBytes);assert.equal(book.size,10977);
 const r=book.lookup(root);assert(['f3-f4','c5-c6'].includes(r.bestMove));assert.equal(r.value,2);assert(r.exact);
 for(const index of [0,100,1000,5000,9000,10976]){
  const sample=toolRow(index),base=book.lookup(sample.position);assert.deepEqual(base.moves[0].cells,sample.cells);assert.equal(base.value,sample.value);
  for(let t=0;t<8;t++){
   const p=rotate(sample.position,t),r=book.lookup(p);
   assert(r.exact);assert.equal(r.value,sample.value);assert(legalMoves(fromOriginal(p)).map(formatMove).includes(r.bestMove));
   assert.deepEqual(r.moves[0].cells,sample.cells.map(a=>cell(a,t)).sort((a,b)=>a-b));
  }
 }
 assert.throws(()=>decodeToolBook(toolBytes.subarray(0,toolBytes.length-1)));
 const bad=Buffer.from(toolBytes);bad[45]=64;assert.throws(()=>decodeToolBook(bad));
});
test('DISPLAY preserves root +2 and exact/bound/estimate values under symmetry',async()=>{
 const ai=await init(read('../docs/play/wasm/dosello_ai.wasm'));await ai.loadBook(displayBytes);
 const r=ai.getBook(root).moves.find(m=>m.move==='f3-f4');assert.equal(r.value,2);assert(r.exact);
 const searched=ai.analyze(root,{bestOnly:true,timeMs:10,useBook:false});
 assert.notEqual(searched.source,'book');assert(legalMoves(initialState()).map(formatMove).includes(searched.bestMove));
 const entries=decodeBook(displayBytes).entries;assert.equal(entries.length,15808);
 for(const kind of ['exact','upper','heuristic']){
  const entry=entries.find(e=>e.analysis.moves.some(m=>m.bound===kind));assert(entry);
  for(let t=0;t<8;t++){
   const p=rotate(entry.position,t),r=ai.getBook(p);assert(r);
   const expected=entry.analysis.moves.map(m=>[m.cells.map(a=>cell(a,t)).sort((a,b)=>a-b).join('-'),m.value,m.lower,m.upper,m.exact,m.depth].join('/')).sort();
   assert.deepEqual(r.moves.map(m=>[m.cells.join('-'),m.value,m.lower,m.upper,m.exact,m.depth].join('/')).sort(),expected);
   assert(r.moves.every(m=>legalMoves(fromOriginal(p)).map(formatMove).includes(m.move)));
  }
 }
});
test('worker keeps TOOL selection independent of DISPLAY and falls back on either missing file',async()=>{
 const source=read('../docs/play/worker.js').toString().replace(/^import .*;\n/gm,'').replaceAll('import.meta.url',"'https://example.invalid/play/worker.js'");
 for(const missingTool of [false,true])for(const missingDisplay of [false,true])for(const bestOnly of [false,true]){
  const messages=[],searches=[];
  const ai={async loadEval3(){},async loadBook(url){assert(url.pathname.endsWith('/display-book.bin'));if(missingDisplay)throw Error('Book HTTP 404');},getBook(){return missingDisplay?null:{moves:[],value:63};},analyze(p,opts){searches.push(opts);return {bestMove:'c3-d3',moves:[]};}};
  const self={postMessage:m=>messages.push(m)};
  const fetcher=async url=>{assert(url.pathname.endsWith('/tool-book.bin'));return {ok:!missingTool,status:missingTool?404:200,arrayBuffer:async()=>toolBytes};};
  runInNewContext(source,{init:async()=>ai,loadToolBook:url=>loadToolBook(url,fetcher),self,URL,fromOriginal});
  await self.onmessage({data:{id:7,position:root,timeMs:3000,bestOnly}});
  assert.deepEqual(messages.map(m=>m.kind),['book','result']);
  assert.equal(Boolean(messages[0].bookError),missingDisplay);
  assert.equal(messages[0].tool===null,missingTool);
  if(bestOnly&&!missingTool){assert.equal(searches.length,0);assert.equal(messages[1].result.source,'tool-book');}
  else {assert.equal(searches.length,1);assert.equal(searches[0].timeMs,3000);assert.equal(searches[0].useBook,!bestOnly);}
 }
});
