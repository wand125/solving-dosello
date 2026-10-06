// All values use the mover's final disc difference, including book bounds.
export const signed=v=>v>0?'+'+v:String(v).replace('-','−');
export const valueKind=m=>m.exact?'exact':Number.isFinite(m.lower)&&m.lower> -64||Number.isFinite(m.upper)&&m.upper<64?'bound':'estimate';
export function valueLabel(m,exact='Exact'){
 if(m.exact)return exact+' '+signed(m.value);
 if(Number.isFinite(m.lower)&&m.lower> -64&&Number.isFinite(m.upper)&&m.upper<64)return '['+signed(m.lower)+', '+signed(m.upper)+']';
 if(Number.isFinite(m.upper)&&m.upper<64)return '≤ '+signed(m.upper);
 if(Number.isFinite(m.lower)&&m.lower> -64)return '≥ '+signed(m.lower);
 return '≈ '+signed(m.value);
}
export function mergeValues(book,result){
 const map=new Map((result?.moves??[]).map(m=>[m.move,{...m}]));
 for(const m of book?.moves??[]){const r=map.get(m.move);if(m.exact||!r?.exact)map.set(m.move,{...r,...m,pv:r?.pv?.length>m.pv?.length?r.pv:m.pv});}
 return [...map.values()];
}
export function provenBest(moves){return moves.filter(m=>m.exact&&moves.every(n=>n.move===m.move||(n.exact?n.value<=m.value:Number.isFinite(n.upper)&&n.upper<=m.value))).map(m=>m.move);}
export function bestMoves(moves){const proven=provenBest(moves);if(proven.length)return proven;const estimates=moves.filter(m=>valueKind(m)==='estimate'&&Number.isFinite(m.value));const candidates=(estimates.length?estimates:moves).filter(m=>Number.isFinite(m.value));const v=Math.max(...candidates.map(m=>m.value));return candidates.filter(m=>m.value===v).map(m=>m.move);}
