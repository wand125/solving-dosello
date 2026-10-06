// Optional match harness: original assets remain in the ignored test cache.
import {spawnSync} from 'node:child_process';
import {initialState,legalMoves,applyMove,pass,isGameOver,score,toOriginal,formatMove} from '../engine/rules.js';
const [gamesText='200',timeText='500',opponent='site5',threadsText='4']=process.argv.slice(2);
const games=Number(gamesText),timeMs=Number(timeText),threads=Number(threadsText);
if(!Number.isInteger(games)||games<2||games%2||!Number.isInteger(timeMs)||timeMs<1||!Number.isInteger(threads)||threads<1||opponent!=='site5')throw Error('Usage: node test/match_eval.js <even games> <ms/move> site5 <threads>');
const {original}=await import('./original-helper.js');
if(!original){console.log('SKIP match_eval: optional original CPU unavailable (offline or download unavailable)');process.exit(0);}
let seed=1940669073;
const random=()=>{seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return (seed>>>0)/4294967296;};
original.random(random);
const binary=new URL('../rust/target/release/analyze',import.meta.url);
const results={games,timeMs,threads,opponent,openingPlies:4,pairedColors:true,book:false,seed, wins:0,draws:0,losses:0,records:[]};
for(let pair=0;pair<games/2;pair++){
 let opening=initialState();const prefix=[];
 for(let ply=0;ply<4;ply++){if(!legalMoves(opening).length)opening=pass(opening);const ms=legalMoves(opening);if(!ms.length)break;const m=ms[Math.floor(random()*ms.length)];prefix.push(formatMove(m));opening=applyMove(opening,m);}
 for(const color of [1,-1]){
  let state=opening;const moves=[...prefix];
  while(!isGameOver(state)){
   const ms=legalMoves(state);if(!ms.length){state=pass(state);moves.push('pass');continue;}
   let move;
   if(state.turn===color){
    const run=spawnSync(binary.pathname,[...moves,'--time',String(timeMs),'--threads',String(threads)],{encoding:'utf8',maxBuffer:8*1024*1024});
    if(run.error||run.status!==0)throw Error('Native analyze failed; build it first. '+(run.error?.message??run.stderr));
    const result=JSON.parse(run.stdout);move=ms.find(m=>formatMove(m)===result.bestMove);
   }else{
    const view=toOriginal(state);original.set(view.shape,5);
    const chosen=original.choose(view.board,state.turn);
    const cells=chosen?.cells?.map(([r,c])=>r*8+c).sort((a,b)=>a-b);
    move=ms.find(m=>cells&&m.cells[0]===cells[0]&&m.cells[1]===cells[1]);
   }
   if(!move)throw Error('CPU returned an illegal move; original API may have changed');
   moves.push(formatMove(move));state=applyMove(state,move);
  }
  const difference=score(state).difference*color;results[difference>0?'wins':difference<0?'losses':'draws']++;
  results.records.push({color,difference,moves});console.error(`Game ${results.records.length}/${games}: ${difference}`);
 }
}
console.log(JSON.stringify(results,null,2));
