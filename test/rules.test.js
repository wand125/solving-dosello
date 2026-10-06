import test from 'node:test';
import assert from 'node:assert/strict';
import {initialState,fromOriginal,toOriginal,legalMoves,applyMove,pass,score,recomputeHash,formatMove,parseMove,switchTurn,isGameOver} from '../engine/rules.js';
import {chooseMove} from '../engine/ai.js';
import {original as o,originalMove} from './original-helper.js';
let seed=872345;
const rng=()=>{seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return (seed>>>0)/4294967296;};
const plain=x=>JSON.parse(JSON.stringify(x));
const canonical=m=>({cells:plain(m.cells),flips:plain(m.flips).map(([r,c])=>r*8+c).sort((a,b)=>a-b)});
test('2000 complete seeded games: all legal moves, all successor boards, pairings and hashes match original',{skip:!o && 'Optional original unavailable (offline)'},()=>{
  for(let game=0;game<2000;game++){
    let s=initialState(),ref=toOriginal(s);
    for(let ply=0;ply<60;ply++){
      const ours=legalMoves(s),theirs=o.moves(ref.board,s.turn,ref.shape);
      assert.deepEqual(ours.map(m=>canonical(originalMove(m))),Array.from(theirs,canonical));
      assert.deepEqual(s.hash,recomputeHash(s));
      assert.deepEqual(toOriginal(fromOriginal(ref)),ref);
      // Every child at every position, not only the randomly selected continuation.
      for(let i=0;i<ours.length;i++){
        const child=applyMove(s,ours[i]),view=toOriginal(child);
        assert.deepEqual(view.board,plain(o.apply(ref.board,theirs[i],s.turn)));
        assert.deepEqual(view.shape,plain(o.nextShape(ref.shape,theirs[i])));
        assert.deepEqual(child.hash,recomputeHash(child));
      }
      if(!ours.length){const opp=o.moves(ref.board,-s.turn,ref.shape);assert.equal(isGameOver(s),!opp.length);if(!opp.length)break;s=pass(s);ref.turn=s.turn;continue;}
      const index=Math.floor(rng()*ours.length);
      ref={board:plain(o.apply(ref.board,theirs[index],s.turn)),shape:plain(o.nextShape(ref.shape,theirs[index])),turn:-s.turn};
      s=applyMove(s,ours[index]);
    }
  }
});
test('notation, mandatory pass, pairing hash and terminal score',()=>{
  const s=initialState();for(const m of legalMoves(s))assert.equal(parseMove(s,formatMove(m)).id,m.id);
  assert.throws(()=>pass(s));assert.throws(()=>parseMove(s,'a1-a2'));
  assert.deepEqual(switchTurn(s).hash,recomputeHash(switchTurn(s)));
  const board=Array.from({length:8},()=>Array(8).fill(1));
  const horizontal=board.map(row=>row.map((_,c)=>c%2?'h-right':'h-left'));
  const vertical=board.map((row,r)=>row.map(()=>r%2?'v-bottom':'v-top'));
  const a=fromOriginal({board,shape:horizontal}),b=fromOriginal({board,shape:vertical});
  assert.notDeepEqual(a.hash,b.hash);assert.equal(score(a).difference,64);assert.equal(chooseMove(a).score,64);
});
function brute(s){const ms=legalMoves(s);if(!ms.length)return isGameOver(s)?score(s).difference*s.turn:-brute(pass(s));return Math.max(...ms.map(m=>-brute(applyMove(s,m))));}
test('exact solver agrees with exhaustive endgames including passes and holes',()=>{
  for(let game=0;game<30;game++){
    let s=initialState();while(score(s).empty>8&&!isGameOver(s)){const ms=legalMoves(s);s=ms.length?applyMove(s,ms[Math.floor(rng()*ms.length)]):pass(s);}
    const result=chooseMove(s,{timeMs:10000});assert.equal(result.solved,true);assert.equal(result.score,brute(s));
    if(result.move)assert.equal(-brute(applyMove(s,result.move)),result.score);
    for(const m of result.pv){if(m){assert(legalMoves(s).some(x=>x.id===m.id));s=applyMove(s,m);}else s=pass(s);}
  }
});
test('partner capture preserves exact semantics even for a mixed-color paired input',{skip:!o && 'Optional original unavailable (offline)'},()=>{
  const ref=toOriginal(initialState());ref.board[3][2]=1;
  const s=fromOriginal(ref),ours=legalMoves(s),theirs=o.moves(ref.board,s.turn,ref.shape);
  assert.deepEqual(ours.map(m=>canonical(originalMove(m))),Array.from(theirs,canonical));
  for(let i=0;i<ours.length;i++){
    const child=applyMove(s,ours[i]);
    assert.deepEqual(toOriginal(child).board,plain(o.apply(ref.board,theirs[i],s.turn)));
    assert.deepEqual(child.hash,recomputeHash(child));
  }
});
test('AI fixed node budgets are repeatable and zero time still supplies a legal move',()=>{
  let s=initialState();
  if(process.env.DOSELLO_TEST_SMALL)while(score(s).empty>28&&!isGameOver(s)){const ms=legalMoves(s);s=ms.length?applyMove(s,ms[0]):pass(s);}
  const a=chooseMove(s,{timeMs:10000,nodeLimit:1000}),b=chooseMove(s,{timeMs:10000,nodeLimit:1000});
  assert.equal(a.move.id,b.move.id);assert.equal(a.score,b.score);assert.equal(a.depth,b.depth);assert.equal(a.nodes,b.nodes);
  assert(legalMoves(s).some(m=>m.id===chooseMove(s,{timeMs:0}).move.id));
});
test('forced pass keeps pairing/hash, consumes no search placement, and terminal holes are unawarded',()=>{
  const board=Array.from({length:8},()=>Array(8).fill(-1));
  const shape=board.map(row=>row.map((_,c)=>c%2?'h-right':'h-left'));
  board[0][0]=board[0][1]=1;board[0][4]=board[0][5]=0;shape[0][4]=shape[0][5]='';
  const s=fromOriginal({board,shape,turn:-1});
  assert.equal(legalMoves(s).length,0);assert.equal(isGameOver(s),false);
  const next=pass(s);assert.equal(next.turn,1);assert.deepEqual(next.hash,recomputeHash(next));
  const result=chooseMove(s,{timeMs:1000});assert.equal(result.move,null);assert.equal(result.solved,true);assert.equal(result.score,brute(s));assert.equal(result.pv[0],null);
  board[0][0]=board[0][1]=-1;
  const terminal=fromOriginal({board,shape});
  assert.equal(isGameOver(terminal),true);assert.deepEqual(score(terminal),{black:0,white:62,empty:2,difference:-62});assert.equal(chooseMove(terminal).score,-62);
});
