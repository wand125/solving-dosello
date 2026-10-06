// Values are always in the current player's perspective.
export const signed=v=>v>0?'+'+v:String(v).replace('-','−');
export function valueLabel(m){
 if(m.exact)return '確定 '+signed(m.value);
 if(Number.isFinite(m.lower)&&m.lower> -64&&Number.isFinite(m.upper)&&m.upper<64)return '['+signed(m.lower)+', '+signed(m.upper)+']';
 if(Number.isFinite(m.upper)&&m.upper<64)return '≤ '+signed(m.upper);
 if(Number.isFinite(m.lower)&&m.lower> -64)return '≥ '+signed(m.lower);
 return '≈ '+signed(m.value);
}
export function mergeValues(book,result){
 const map=new Map((result?.moves??[]).map(m=>[m.move,{...m}]));
 for(const m of book?.moves??[]){
  const r=map.get(m.move);
  // Proof bounds take display precedence over time-limited estimates.
  if(m.exact||!r?.exact)map.set(m.move,{...m});
 }
 return [...map.values()];
}
export function bestMoves(moves){
 if(!moves.length)return [];
 const proven=moves.filter(m=>m.exact&&moves.every(n=>n.move===m.move||(Number.isFinite(n.upper)?n.upper<=m.value:n.exact&&n.value<=m.value)));
 if(proven.length)return proven.map(m=>m.move);
 const v=Math.max(...moves.map(m=>m.value));return moves.filter(m=>m.value===v).map(m=>m.move);
}
