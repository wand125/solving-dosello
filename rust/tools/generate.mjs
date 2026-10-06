import {original} from '../../test/original-helper.js';
if(!original){console.error('SKIP: optional original unavailable');process.exit(0);}
import {initialState,toOriginal} from '../../engine/rules.js';
import {once} from 'node:events';
let seed=0x73ac4291;function random(){seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return (seed>>>0)/4294967296;}
const games=+(process.argv[2]??2000);let count=0;
for(let game=0;game<games;game++){
 let {board,shape,turn}=toOriginal(initialState());
 for(let ply=0;ply<60;ply++){
  const ms=original.moves(board,turn,shape);
  const record={board,shape,turn,moves:ms.map(m=>({cells:m.cells.map(([r,c])=>r*8+c),flips:m.flips.map(([r,c])=>r*8+c).sort((a,b)=>a-b),next:{board:original.apply(board,m,turn),shape:original.nextShape(shape,m),turn:-turn}}))};
  if(!process.stdout.write(JSON.stringify(record)+'\n'))await once(process.stdout,'drain');count++;
  if(!ms.length){if(!original.moves(board,-turn,shape).length)break;turn=-turn;continue;}
  const m=ms[Math.floor(random()*ms.length)];board=original.apply(board,m,turn);shape=original.nextShape(shape,m);turn=-turn;
 }
}
console.error(`Generated ${games} original games, ${count} positions`);
