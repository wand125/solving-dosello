import {legalMoves,applyMove,switchTurn,score,NEIGHBORS,ORTHOGONAL} from './rules.js';

// All evaluation weights live here. Integer units; final results use 1000/cell.
export const WEIGHTS=Object.freeze({discEarly:-2,discLate:18,corner:190,edge:24,
  danger:-65,mobility:18,potential:5,frontier:-9,stable:90,regionParity:5,terminal:1000});
export const ENDGAME_THRESHOLD=12;
const corners=[0,7,56,63];
const edge=i=>(i>>3)===0||(i>>3)===7||(i&7)===0||(i&7)===7;
const timeout=Symbol('timeout');

export function evaluate(s,ownMoves){
  const w=WEIGHTS,b=s.board,p=s.turn,n=score(s),phase=(56-n.empty)/56;
  let value=n.difference*p*(w.discEarly+(w.discLate-w.discEarly)*phase*phase);
  let frontier=0,potential=0;
  for(let i=0;i<64;i++){
    if(b[i]){if(edge(i))value+=b[i]*p*w.edge;if(NEIGHBORS[i].some(j=>!b[j]))frontier+=b[i]*p;}
    else {let own=false,opp=false;for(const j of NEIGHBORS[i]){if(b[j]===p)own=true;if(b[j]===-p)opp=true;}potential+=Number(opp)-Number(own);}
  }
  for(const c of corners){
    value+=b[c]*p*w.corner;
    if(!b[c])for(const i of NEIGHBORS[c])value+=b[i]*p*w.danger;
  }
  // A corner alone is not stable in this variant: its partner can be captured.
  // Conservative stable edge runs: both cells of a domino must be protected.
  const protectedCells=new Uint8Array(64);
  for(const c of corners)if(b[c]){
    protectedCells[c]=1;
    const steps=[(c&7)===0?1:-1,c<8?8:-8];
    for(const d of steps)for(let i=c+d,k=1;k<8&&b[i]===b[c];i+=d,k++)protectedCells[i]=1;
  }
  for(let i=0;i<64;i++)if(b[i]&&protectedCells[i]&&protectedCells[s.pair[i]])value+=b[i]*p*w.stable;
  value+=frontier*w.frontier+potential*w.potential;
  value+=((ownMoves??legalMoves(s)).length-legalMoves(s,-p).length)*w.mobility;
  if(n.empty<=24){
    const seen=new Uint8Array(64);
    for(let i=0;i<64;i++)if(!b[i]&&!seen[i]){
      const queue=[i];seen[i]=1;
      for(let k=0;k<queue.length;k++)for(const j of ORTHOGONAL[queue[k]])if(!b[j]&&!seen[j]){seen[j]=1;queue.push(j);}
      // A singleton contributes no playable domino. Larger regions are an estimate,
      // not an assumption used to prune: geometry can leave more holes.
      if(queue.length>1)value+=(Math.floor(queue.length/2)%2?1:-1)*w.regionParity;
    }
  }
  return Math.round(value);
}
function key(s){return s.hash[0]+':'+s.hash[1];}
function cheap(s,m){
  let v=0;
  for(const i of m.cells){if(corners.includes(i))v+=600;else if(edge(i))v+=60;for(const c of corners)if(!s.board[c]&&NEIGHBORS[c].includes(i))v-=160;}
  return v-m.flips.length;
}
/** Deterministic nodeLimit/maxDepth modes are provided for reproducible benchmarks.
 * timeMs remains a hard wall-clock budget; incomplete iterations are discarded.
 */
export function chooseMove(root,{timeMs=1000,maxDepth=60,nodeLimit=Infinity,endgameThreshold=ENDGAME_THRESHOLD}={}){
  if(!Number.isFinite(timeMs)||timeMs<0||!(maxDepth>=1)||!(nodeLimit>=1))throw Error('Invalid search budget');
  const started=performance.now(),deadline=started+timeMs;
  const tt=new Map(),history=new Int32Array(8192),killers=[];
  let nodes=0,completedDepth=0,bestScore=0,solved=false;
  const options=legalMoves(root),empty=score(root).empty;
  let best=options[0]??null,pv=best?[best]:[];
  function check(){if(nodes>=nodeLimit||performance.now()>=deadline)throw timeout;}
  function negamax(s,depth,alpha,beta,ply){
    nodes++;if((nodes&31)===0||nodes>=nodeLimit)check();
    const originalAlpha=alpha,k=key(s),hit=tt.get(k);
    if(hit&&hit.depth>=depth){if(hit.flag===0)return hit.value;if(hit.flag===1)alpha=Math.max(alpha,hit.value);else beta=Math.min(beta,hit.value);if(alpha>=beta)return hit.value;}
    const moves=legalMoves(s);
    if(!moves.length){
      if(!legalMoves(s,-s.turn).length)return score(s).difference*s.turn*WEIGHTS.terminal;
      return -negamax(switchTurn(s),depth,-beta,-alpha,ply+1);
    }
    if(depth<=0)return evaluate(s,moves);
    const hOffset=s.turn===1?4096:0;
    const ranked=moves.map(m=>({m,rank:m.id===hit?.move?1e9:m.id===killers[ply]?1e8:history[hOffset+m.id]+cheap(s,m)}));
    ranked.sort((a,b)=>b.rank-a.rank);
    let value=-Infinity,bestId=ranked[0].m.id;
    for(let i=0;i<ranked.length;i++){
      const m=ranked[i].m,next=applyMove(s,m);
      let v;
      if(i===0)v=-negamax(next,depth-1,-beta,-alpha,ply+1);
      else {v=-negamax(next,depth-1,-alpha-1,-alpha,ply+1);if(v>alpha&&v<beta)v=-negamax(next,depth-1,-beta,-alpha,ply+1);}
      if(v>value){value=v;bestId=m.id;}
      alpha=Math.max(alpha,v);
      if(alpha>=beta){killers[ply]=m.id;history[hOffset+m.id]+=depth*depth;break;}
    }
    if(tt.size>200000)tt.clear();
    tt.set(k,{depth,value,move:bestId,flag:value<=originalAlpha?2:value>=beta?1:0});
    return value;
  }
  function principalVariation(depth){
    const line=[];let s=root;
    for(let d=0;d<depth;){
      const ms=legalMoves(s);
      if(!ms.length){if(!legalMoves(s,-s.turn).length)break;line.push(null);s=switchTurn(s);continue;}
      const m=ms.find(m=>m.id===tt.get(key(s))?.move);if(!m)break;line.push(m);s=applyMove(s,m);d++;
    }
    return line;
  }
  if(!options.length&&!legalMoves(root,-root.turn).length){return {move:null,score:score(root).difference*root.turn,scoreType:'exact',depth:0,nodes:0,pv:[],solved:true,elapsedMs:performance.now()-started};}
  // Outside the solving zone cap at normal depth; inside it, continue all the way
  // to floor(empty/2). Passes do not consume placement depth.
  const fullDepth=Math.floor(empty/2),limit=Math.min(maxDepth,empty<=endgameThreshold?fullDepth:Math.min(12,fullDepth));
  for(let depth=1;depth<=limit;depth++){
    try{
      check();const value=negamax(root,depth,-Infinity,Infinity,0);check();
      const line=principalVariation(depth);
      best=options.length?(options.find(m=>m.id===tt.get(key(root))?.move)??best):null;
      bestScore=value;completedDepth=depth;pv=line.length?line:(best?[best]:[null]);
      if(depth>=fullDepth){solved=true;break;}
    }catch(e){if(e!==timeout)throw e;break;}
  }
  // Zero-budget fallback is legal and uses the same evaluation scale.
  if(!completedDepth)bestScore=evaluate(root,options);
  return {move:best,score:solved?bestScore/WEIGHTS.terminal:bestScore,scoreType:solved?'exact':'heuristic',depth:completedDepth,nodes,pv,solved,elapsedMs:performance.now()-started};
}
