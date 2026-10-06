import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import init from '../docs/play/wasm/dosello.js';
import {reviewValue,lossBetween,lossLabel,lossColor,firstDecisive,reviewSummary,canonicalPosition,ReviewCache,reviewGame,reviewDefaults} from '../docs/play/review.js';
import {parseSequence} from '../docs/play/record.js';
import {initialState,toOriginal,fromOriginal,score,legalMoves,applyMove,formatMove} from '../docs/play/engine/rules.js';
const exact=value=>reviewValue({value,exact:true}),bound=(lower,upper)=>reviewValue({value:lower,lower,upper}),estimate=value=>reviewValue({value});
test('loss interval arithmetic preserves exact, bound and estimate kinds',()=>{
 for(const [a,b,kind,lo,hi,label] of [
  [exact(2),exact(-2),'exact',4,4,'4'],[exact(2),bound(-64,-2),'bound',4,66,'≥ 4'],
  [exact(2),bound(0,2),'bound',0,2,'≤ 2'],[bound(2,4),bound(-2,0),'bound',2,6,'[2, 6]'],
  [bound(0,2),exact(2),'exact',0,0,'0'],[bound(2,6),exact(0),'bound',2,6,'[2, 6]'],
 ]){const l=lossBetween(a,b);assert.equal(l.kind,kind);assert.equal(l.lower,lo);assert.equal(l.upper,hi);assert.equal(lossLabel(l),label);}
 for(const [a,b] of [[estimate(2),exact(-1)],[exact(2),estimate(-1)],[estimate(2),estimate(-1)],[bound(2,4),estimate(-1)],[estimate(2),bound(-1,0)]]){const l=lossBetween(a,b);assert.equal(l.kind,'estimate');assert.equal(lossLabel(l),'≈ 3');}
 assert.equal(lossBetween(estimate(-8),estimate(2)).value,0);
 assert.equal(lossBetween(null,exact(2)),null);
});
test('color uses proven minimum and estimates have a separate style',()=>{
 for(const [l,c] of [[lossBetween(exact(2),exact(2)),'loss-zero'],[lossBetween(exact(2),exact(0)),'loss-yellow'],[lossBetween(exact(2),bound(-64,-2)),'loss-red'],[lossBetween(exact(2),bound(-64,2)),'loss-neutral'],[lossBetween(estimate(2),exact(-2)),'estimated loss-red']])assert.equal(lossColor(l),c);
});
test('decisive mistakes and totals use only proven intervals in mover perspective',()=>{
 const rows=[{side:1,best:estimate(2),played:exact(-2)},{side:-1,best:bound(-2,2),played:exact(-2)},{side:1,best:exact(2),played:bound(-64,-2)}];
 rows.forEach(r=>r.loss=lossBetween(r.best,r.played));
 assert.equal(firstDecisive(rows),2);assert.equal(reviewSummary(rows).sides[1].total,4);assert.equal(reviewSummary(rows).sides[1].mistakes,1);
 assert.equal(firstDecisive([{best:exact(0),played:bound(-4,-2)}]),0);
 assert.equal(firstDecisive([{best:exact(2),played:bound(0,2)}]),-1);
 assert.equal(firstDecisive([{best:exact(2),played:bound(-4,0)}]),0);
 assert.equal(firstDecisive([{best:bound(0,2),played:bound(-4,-2)}]),0);
 assert.equal(firstDecisive([{best:exact(-2),played:exact(-8)}]),-1);
});
test('canonical cache includes pairs and turn and maps rotated best moves back',()=>{
 const {states,moves}=parseSequence('f3-f4 e6-f6'),s=states[1];
 const rotated={board:new Int8Array(64),pair:new Int8Array(64).fill(-1),turn:s.turn};
 for(let i=0;i<64;i++){rotated.board[63-i]=s.board[i];if(s.board[i])rotated.pair[63-i]=63-s.pair[i];}
 assert.equal(canonicalPosition(s).key,canonicalPosition(rotated).key);
 assert.notEqual(canonicalPosition(s).key,canonicalPosition({...s,turn:-s.turn}).key);
 const other={...s,pair:s.pair.slice()};other.pair[20]=21;assert.notEqual(canonicalPosition(s).key,canonicalPosition(other).key);
 const cache=new ReviewCache(),row={best:exact(-2),played:exact(-2),bestMoves:[moves[1]],side:-1,done:true};cache.set(s,moves[1],row);
 assert.deepEqual(cache.get(rotated,'c3-d3').bestMoves,['c3-d3']);
 assert.deepEqual(new ReviewCache(cache.entries()).get(s,moves[1]),row);
 assert.equal(cache.get(s,'a1-b1'),null);
});
const wasm=readFileSync(new URL('../docs/play/wasm/dosello_ai.wasm',import.meta.url));
const book=readFileSync(new URL('../docs/play/wasm/opening-book.bin',import.meta.url));
test('full saved perfect line has zero proven loss; cached reopening does no engine work',async t=>{
 const ai=await init(wasm);await ai.loadBook(book);
 const records=readFileSync(new URL('../docs/data/perfect-line.jsonl',import.meta.url),'utf8').trim().split('\n').map(JSON.parse);
 const game=parseSequence(records.find(r=>r.seq).seq.join(' ')),cache=new ReviewCache();
 const start=performance.now(),rows=reviewGame(ai,game.states,game.moves,{cache});
 t.diagnostic(`Perfect-line review: ${(performance.now()-start).toFixed(2)} ms, ${rows.length} moves, ${rows.filter(r=>r.loss?.kind==='exact').length} exact, ${rows.filter(r=>r.source==='book').length} book`);
 for(const r of rows){assert(r.done);assert(r.loss);if(r.loss.kind!=='estimate'){assert.equal(r.loss.lower,0);if(r.loss.kind==='exact')assert.equal(r.loss.value,0);}}
 assert(rows.some(r=>r.loss.kind==='exact')); assert(rows.some(r=>r.source==='solved'));
 assert.equal(firstDecisive(rows),-1);assert.equal(reviewSummary(rows).sides[1].mistakes+reviewSummary(rows).sides[-1].mistakes,0);
 assert.deepEqual(reviewGame({getBook(){throw Error('cache miss');}},game.states,game.moves,{cache}),rows);
});
test('Black a4-b4 is immediately a proven ≥ 4 loss from the bundled book',async()=>{
 const ai=await init(wasm);await ai.loadBook(book);ai.analyze=()=>{throw Error('unneeded solve');};
 const game=parseSequence('a4-b4'),[r]=reviewGame(ai,game.states,game.moves);
 assert.equal(r.source,'book');assert.equal(r.best.kind,'exact');assert.equal(r.best.value,2);assert.equal(r.played.kind,'bound');assert.equal(lossLabel(r.loss),'≥ 4');assert.equal(r.loss.kind,'bound');assert.equal(firstDecisive([r]),0);
});
test('targeted WASM proof and played-child value agree with exhaustive small endgame analysis',async()=>{
 const ai=await init(wasm),game=parseSequence('f3-f4 e6-f6 d7-e7');
 let s=game.states.at(-1);while(score(s).empty>12&&legalMoves(s).length)s=applyMove(s,legalMoves(s)[0]);
 assert(legalMoves(s).length);
 const all=ai.analyze(toOriginal(s),{timeMs:5000});assert(all.moves.every(m=>m.exact));
 const chosen=all.moves.at(-1),rows=reviewGame(ai,[s],[chosen.move],{exactMs:5000});
 assert.equal(rows[0].best.value,Math.max(...all.moves.map(m=>m.value)));assert.equal(rows[0].played.value,chosen.value);assert.equal(rows[0].loss.kind,'exact');
 const proof=ai.analyze(toOriginal(s),{reviewSolve:true,timeMs:5000});assert(proof.exact);assert.equal(proof.complete,false);assert.equal(proof.moves,undefined);
});
test('book pass precedes solves, phone budgets, timeout fallback stays estimated',()=>{
 assert.deepEqual(reviewDefaults(true),{exactEmpties:26,estimateMs:500,exactMs:1000});
 const game=parseSequence('f3-f4 e6-f6'),events=[],calls=[];
 const ai={getBook:()=>null,analyze(p,o){calls.push(o);events.push('solve');if(o.reviewSolve)return {exact:false,value:null};const m=formatMove(legalMoves(fromOriginal(p))[0]);return {bestMove:m,moves:[{move:m,value:2,exact:false}]};}};
 const rows=reviewGame(ai,game.states,game.moves,{exactEmpties:56,onRow:i=>events.push(i)});
 assert.deepEqual(events.slice(0,2),[0,1]);assert(rows.every(r=>r.loss.kind==='estimate'));assert(calls.every(c=>c.bestOnly));assert(calls.some(c=>c.valueOnly&&c.reviewSolve));
});

test('an estimated candidate cannot inherit an exact root value as a proven played value',()=>{
 const game=parseSequence('a4-b4'),ai={
  getBook(p){return score(fromOriginal(p)).empty===56?{exact:true,value:2,moves:[]}:null;},
  analyze(p){const root=score(fromOriginal(p)).empty===56,move=root?'a4-b4':formatMove(legalMoves(fromOriginal(p))[0]);return {bestMove:move,moves:[{move,value:root?2:6,exact:false}]};}
 };
 const [r]=reviewGame(ai,game.states,game.moves);assert.equal(r.best.kind,'exact');assert.equal(r.bestMovesProven,false);assert.equal(r.played.kind,'estimate');assert.equal(r.loss.kind,'estimate');
});

test('a played-child book proof takes priority over a parent heuristic',()=>{
 const ai={getBook(p){return score(fromOriginal(p)).empty===56?{exact:true,value:2,moves:[{move:'f3-f4',value:2,exact:true},{move:'c5-c6',value:2}]}:{exact:true,value:-2,moves:[]};},analyze(){throw Error('book proof should suffice');}};
 // Use the symmetric legal opening whose parent entry is merely heuristic.
 const other=parseSequence('c5-c6'),[r]=reviewGame(ai,other.states,other.moves);
 assert.equal(r.played.kind,'exact');assert.equal(r.played.value,2);assert.equal(r.loss.value,0);
});
