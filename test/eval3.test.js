import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import init from '../docs/play/wasm/dosello.js';
import {initialState,toOriginal,legalMoves,formatMove} from '../engine/rules.js';
const read=p=>readFileSync(new URL(p,import.meta.url));
test('shipped eval3 weights load in WASM, search legally, and preserve the proven +2 book',async()=>{
 const wasm=read('../docs/play/wasm/dosello_ai.wasm');
 const weights=read('../docs/play/wasm/eval3-r2.bin');
 assert.deepEqual(weights,read('../rust/data/eval3-r2.bin'));
 const ai=await init(wasm);
 await assert.rejects(ai.loadEval3(new Uint8Array([1,2,3])),/Invalid eval3/);
 const position=toOriginal(initialState());
 const legal=legalMoves(initialState()).map(formatMove);
 assert(legal.includes(ai.analyze(position,{timeMs:20,bestOnly:true}).bestMove));
 await ai.loadEval3(weights);
 const result=ai.analyze(position,{timeMs:100,bestOnly:true});
 assert(legal.includes(result.bestMove));
 await assert.rejects(ai.loadEval3(new Uint8Array([1,2,3])),/Invalid eval3/);
 assert(legal.includes(ai.analyze(position,{timeMs:20,bestOnly:true}).bestMove));
 await ai.loadBook(read('../docs/play/wasm/opening-book.bin'));
 const move=ai.getBook(position).moves.find(m=>m.move==='f3-f4');
 assert.equal(move.value,2);assert.equal(move.exact,true);
 const chosen=ai.analyze(position,{timeMs:3000,bestOnly:true});
 assert.equal(chosen.source,'book');assert.equal(chosen.value,2);
 assert(['f3-f4','c5-c6'].includes(chosen.bestMove));
 // Loading also resets any opt-in experimental policy to plain eval3.
 const {instance:{exports:e}}=await WebAssembly.instantiate(wasm,{env:{now_ms:()=>performance.now()}});
 assert.equal(e.set_eval3_ordering(1),0);
 const ptr=e.alloc(weights.length);
 new Uint8Array(e.memory.buffer,ptr,weights.length).set(weights);
 assert.equal(e.load_eval3(ptr,weights.length),0);
 e.free(ptr,weights.length);
 assert.equal(e.load_eval3(0,0),0);
});
test('worker awaits weights before search and quietly falls back on fetch failure',async()=>{
 const source=read('../docs/play/worker.js').toString().replace(/^import .*;\n/gm,'');
 for(const fails of [false,true]){
  const calls=[],messages=[];
  const ai={async loadEval3(){calls.push('weights');await Promise.resolve();if(fails)throw Error('unavailable');calls.push('loaded');},async loadBook(){calls.push('book');},getBook(){return null;},analyze(){calls.push('search');return {bestMove:'f3-f4'};}};
  const self={postMessage:m=>messages.push(m)};
  runInNewContext(source.replaceAll('import.meta.url',"'https://example.invalid/play/worker.js'"),{init:async()=>ai,self,URL});
  await self.onmessage({data:{id:1,position:'',timeMs:3000,bestOnly:true}});
  assert.deepEqual(calls,fails?['weights','book','search']:['weights','loaded','book','search']);
  assert.deepEqual(messages.map(m=>m.kind),['book','result']);
  assert.equal(messages[0].bookError,null);
 }
});
