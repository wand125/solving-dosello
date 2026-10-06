/** Pure public API. Squares are r*8+c; moves contain cells:[a,b], flips:[...]. */
export const BLACK = 1, WHITE = -1;
export const RAYS = Array.from({length:64}, (_,i) => {
  const rays=[];
  for(let dr=-1;dr<=1;dr++) for(let dc=-1;dc<=1;dc++) {
    if(!dr&&!dc) continue;
    const ray=[];
    for(let r=(i>>3)+dr,c=(i&7)+dc;r>=0&&r<8&&c>=0&&c<8;r+=dr,c+=dc) ray.push(r*8+c);
    rays.push(ray);
  }
  return rays;
});
export const NEIGHBORS = RAYS.map(rs=>rs.filter(r=>r.length).map(r=>r[0]));
export const ORTHOGONAL = Array.from({length:64},(_,i)=>[i-8,i+8,i-1,i+1].filter(j=>j>=0&&j<64&&Math.abs((i>>3)-(j>>3))+Math.abs((i&7)-(j&7))===1));
const EDGES=[];
for(let i=0;i<64;i++){if((i&7)<7) EDGES.push([i,i+1]);if(i<56) EDGES.push([i,i+8]);}
let seed=0x381acf21;
function random32(){seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return seed>>>0;}
// Independent two-word keys: square/color, square/partner, side to move.
const COLORS=Array.from({length:128},()=>[random32(),random32()]);
const PAIRS=Array.from({length:4096},()=>[random32(),random32()]);
const SIDE=[random32(),random32()];
export function recomputeHash(s){
  let lo=0,hi=0;
  for(let i=0;i<64;i++) if(s.board[i]){
    const c=COLORS[i*2+(s.board[i]===1?1:0)],p=PAIRS[i*64+s.pair[i]];
    lo^=c[0]^p[0];hi^=c[1]^p[1];
  }
  if(s.turn===-1){lo^=SIDE[0];hi^=SIDE[1];}
  return [lo>>>0,hi>>>0];
}
export function fromOriginal({board,shape,turn=BLACK}){
  if(![BLACK,WHITE].includes(turn)||board?.length!==8||shape?.length!==8) throw Error('Invalid position');
  const b=new Int8Array(64),pair=new Int8Array(64).fill(-1);
  const offsets={'h-left':1,'h-right':-1,'v-top':8,'v-bottom':-8};
  for(let r=0;r<8;r++){
    if(board[r]?.length!==8||shape[r]?.length!==8) throw Error('Invalid row');
    for(let c=0;c<8;c++){
      const i=r*8+c,v=board[r][c];
      if(![0,1,-1].includes(v)) throw Error('Invalid cell');
      b[i]=v;
      if(v){const d=offsets[shape[r][c]];if(d===undefined)throw Error('Missing domino partner');pair[i]=i+d;}
      else if(shape[r][c]) throw Error('Shape on empty cell');
    }
  }
  for(let i=0;i<64;i++) if(b[i]&&(pair[i]<0||pair[i]>=64||!ORTHOGONAL[i].includes(pair[i])||pair[pair[i]]!==i||!b[pair[i]]))throw Error('Invalid pairing');
  const s={board:b,pair,turn};s.hash=recomputeHash(s);return s;
}
export function toOriginal(s){
  const board=Array.from({length:8},()=>Array(8).fill(0)),shape=Array.from({length:8},()=>Array(8).fill(''));
  for(let i=0;i<64;i++){board[i>>3][i&7]=s.board[i];if(s.board[i])shape[i>>3][i&7]=({'1':'h-left','-1':'h-right','8':'v-top','-8':'v-bottom'})[s.pair[i]-i];}
  return {board,shape,turn:s.turn};
}
export function initialState(){
  const board=Array.from({length:8},()=>Array(8).fill(0)),shape=Array.from({length:8},()=>Array(8).fill(''));
  for(const [a,b,p] of [[26,27,-1],[36,37,-1],[20,28,1],[35,43,1]]){
    board[a>>3][a&7]=board[b>>3][b&7]=p;
    shape[a>>3][a&7]=b-a===1?'h-left':'v-top';shape[b>>3][b&7]=b-a===1?'h-right':'v-bottom';
  }
  return fromOriginal({board,shape,turn:BLACK});
}
function collect(b,pair,a,z,p){
  let lo=0,hi=0;
  for(const start of [a,z]) for(const ray of RAYS[start]){
    let k=0;while(k<ray.length&&b[ray[k]]===-p) k++;
    if(k&&k<ray.length&&b[ray[k]]===p)for(let j=0;j<k;j++){const i=ray[j];if(i<32)lo|=1<<i;else hi|=1<<(i-32);}
  }
  if(!(lo||hi))return null;
  const flips=[];
  for(let i=0;i<64;i++)if(i<32?lo&(1<<i):hi&(1<<(i-32)))flips.push(i);
  // Partners are added only after collecting all direct captures (no cascading).
  for(let n=flips.length,k=0;k<n;k++){
    const i=pair[flips[k]];
    if(i<32){if(!(lo&(1<<i))){lo|=1<<i;flips.push(i);}}
    else if(!(hi&(1<<(i-32)))){hi|=1<<(i-32);flips.push(i);}
  }
  return {cells:[a,z],flips,id:a*64+z};
}
export function legalMoves(s,p=s.turn){
  const b=s.board.slice(),out=[];
  for(const [a,z]of EDGES)if(!b[a]&&!b[z]){
    b[a]=b[z]=p;const m=collect(b,s.pair,a,z,p);b[a]=b[z]=0;if(m)out.push(m);
  }
  return out;
}
/** applyMove expects a move returned by legalMoves; turn switches, without auto-pass. */
export function applyMove(s,m){
  const board=s.board.slice(),pair=s.pair.slice(),p=s.turn;
  let [lo,hi]=s.hash;
  for(const i of [...m.cells,...m.flips]){
    if(board[i]){const old=COLORS[i*2+(board[i]===1?1:0)];lo^=old[0];hi^=old[1];}
    const next=COLORS[i*2+(p===1?1:0)];lo^=next[0];hi^=next[1];board[i]=p;
  }
  const [a,b]=m.cells;pair[a]=b;pair[b]=a;
  for(const i of m.cells){const k=PAIRS[i*64+pair[i]];lo^=k[0];hi^=k[1];}
  return {board,pair,turn:-p,hash:[(lo^SIDE[0])>>>0,(hi^SIDE[1])>>>0]};
}
/** Search/internal side switch. Public pass() validates that a pass is mandatory. */
export function switchTurn(s){return {...s,turn:-s.turn,hash:[(s.hash[0]^SIDE[0])>>>0,(s.hash[1]^SIDE[1])>>>0]};}
export function isGameOver(s){return !legalMoves(s).length&&!legalMoves(s,-s.turn).length;}
export function pass(s){if(legalMoves(s).length)throw Error('Cannot pass with a legal move');return isGameOver(s)?s:switchTurn(s);}
export function score(s){let black=0,white=0;for(const v of s.board){if(v===1)black++;else if(v===-1)white++;}return {black,white,empty:64-black-white,difference:black-white};}
export function formatMove(m){return m?m.cells.map(i=>'abcdefgh'[i&7]+((i>>3)+1)).join('-'):'pass';}
export function parseMove(s,text){
  if(text==='pass'){if(legalMoves(s).length)throw Error('Illegal pass');return null;}
  if(!/^[a-h][1-8]-[a-h][1-8]$/i.test(text))throw Error('Expected a1-b1');
  const cells=text.toLowerCase().split('-').map(x=>(+x[1]-1)*8+x.charCodeAt(0)-97).sort((a,b)=>a-b);
  const m=legalMoves(s).find(m=>m.cells[0]===cells[0]&&m.cells[1]===cells[1]);if(!m)throw Error('Illegal move: '+text);return m;
}
