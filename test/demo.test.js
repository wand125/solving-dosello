import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import init from '../docs/play/wasm/dosello.js';
import {initialState,toOriginal,legalMoves,formatMove,applyMove,score,pass,isGameOver} from '../engine/rules.js';
import {valueLabel,mergeValues,bestMoves} from '../docs/play/analysis.js';
const read=p=>readFileSync(new URL(p,import.meta.url));
test('published rules match the independently packaged engine',()=>assert.deepEqual(read('../engine/rules.js'),read('../docs/play/engine/rules.js')));
test('WASM searches legally without a book, then compact book proves +2',async()=>{
 const ai=await init(read('../docs/play/wasm/dosello_ai.wasm'));
 const p=toOriginal(initialState()),r=ai.analyze(p,{timeMs:50});
 assert(legalMoves(initialState()).some(m=>formatMove(m)===r.bestMove));assert.equal(r.moves.length,20);
 await ai.loadBook(read('../docs/play/wasm/opening-book.bin'));
 const book=ai.getBook(p);assert.equal(book.value,2);assert.equal(book.exact,true);
 for(const name of ['f3-f4','c5-c6']){const m=book.moves.find(m=>m.move===name);assert(m.exact);assert.equal(m.value,2);assert.equal(valueLabel(m),'確定 +2');}
 const merged=mergeValues(book,r);assert.deepEqual(bestMoves(merged).sort(),['c5-c6','f3-f4']);
 for(const m of merged)if(!['f3-f4','c5-c6','c3-d3','e6-f6'].includes(m.move))assert.equal(valueLabel(m),'≤ −2');
 assert(['f3-f4','c5-c6'].includes(ai.analyze(p,{bestOnly:true}).bestMove));
 const next=applyMove(initialState(),legalMoves(initialState()).find(m=>formatMove(m)==='f3-f4'));
 const reply=ai.getBook(toOriginal(next));assert.equal(reply.moves.find(m=>m.move==='e6-f6').value,-2);
 assert(legalMoves(next).some(m=>formatMove(m)===ai.analyze(toOriginal(next),{bestOnly:true,timeMs:30}).bestMove));
 const proof=JSON.parse(read('../proof/initial-proof.json'));
 for(const n of Object.values(proof.nodes)){const b=ai.getBook(n.position);assert(b);assert(b.lower>=n.lower&&b.upper<=n.upper);}
});
test('bounds keep their direction and are never relabeled as estimates',()=>{
 assert.equal(valueLabel({exact:false,value:0,lower:6,upper:64}),'≥ +6');
 assert.equal(valueLabel({exact:false,value:4}),'≈ +4');
 assert.equal(valueLabel({exact:false,lower:2,upper:6}),'[+2, +6]');
 assert.deepEqual(bestMoves([{move:'a',exact:true,value:2},{move:'b',upper:-2,value:-2}]),['a']);
});
test('WASM and JS rules agree through seeded complete games, including pass handling',async()=>{
 const ai=await init(read('../docs/play/wasm/dosello_ai.wasm'));let seed=654321;
 for(let game=0;game<12;game++){
  let s=initialState();while(!isGameOver(s)){
   const ms=legalMoves(s);if(!ms.length){s=pass(s);continue;}
   const r=ai.analyze(toOriginal(s),{timeMs:0,exactIfPossible:false});
   assert.deepEqual(r.moves.map(m=>m.move).sort(),ms.map(formatMove).sort());
   seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;s=applyMove(s,ms[(seed>>>0)%ms.length]);
  }assert(score(s).empty>=0);
 }
});
