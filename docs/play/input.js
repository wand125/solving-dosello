// Independent placement input; coordinates are measured in cell widths.
import {legalMoves,ORTHOGONAL} from './engine/rules.js';
export function dragTarget(start,origin,point){
 const col=Math.floor(point.x),row=Math.floor(point.y);
 const over=col>=0&&col<8&&row>=0&&row<8?row*8+col:null;
 if(ORTHOGONAL[start]?.includes(over))return over;
 const dx=point.x-origin.x,dy=point.y-origin.y;
 if(Math.max(Math.abs(dx),Math.abs(dy))<.45||Math.abs(dx)===Math.abs(dy))return null;
 const dc=Math.abs(dx)>Math.abs(dy)?Math.sign(dx):0,dr=dc?0:Math.sign(dy);
 const c=start%8+dc,r=(start>>3)+dr;
 return c>=0&&c<8&&r>=0&&r<8?r*8+c:null;
}
export function previewMove(state,start,target){
 if(!ORTHOGONAL[start]?.includes(target)||state.board[start]||state.board[target])return null;
 return legalMoves(state).find(m=>m.cells.includes(start)&&m.cells.includes(target))??null;
}
export function keyboardMove(state,start,key){
 const delta={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[key];
 if(!delta||!Number.isInteger(start)||start<0||start>63)return null;
 const c=start%8+delta[0],r=(start>>3)+delta[1];
 return c>=0&&c<8&&r>=0&&r<8?previewMove(state,start,r*8+c):null;
}
export function placementEnabled(mode,turn,over=false){return !over&&(mode==='two'||turn===(mode==='black'?1:-1));}
export function bindPlacement(board,{state,enabled,play,select}){
 let drag=null;
 const point=e=>{const p=new DOMPoint(e.clientX,e.clientY).matrixTransform(board.getScreenCTM().inverse());return {x:(p.x-30)/50,y:(p.y-30)/50};};
 function repaint(){
  board.querySelector('.placement-preview')?.remove();
  if(!drag||!drag.moved)return;
  const {start,target}=drag,a=target===null?start:Math.min(start,target),b=target===null?start:Math.max(start,target);
  const legal=previewMove(state(),start,target),rect=document.createElementNS(board.namespaceURI,'rect');
  const attrs={class:'placement-preview',x:30+a%8*50+4,y:30+(a>>3)*50+4,width:b-a===1?92:42,height:b-a===8?92:42,rx:7,fill:legal?(state().turn===1?'#292c32':'#fafafa'):'none',stroke:legal?'#657dab':'#d32636','stroke-width':4,'pointer-events':'none','data-legal':String(Boolean(legal))};
  for(const [k,v] of Object.entries(attrs))rect.setAttribute(k,v);
  board.append(rect);
 }
 function cancel(){const old=drag;drag=null;board.querySelector('.placement-preview')?.remove();if(old&&board.hasPointerCapture(old.id))board.releasePointerCapture(old.id);}
 function update(e){const p=point(e);drag.moved ||= Math.max(Math.abs(p.x-drag.origin.x),Math.abs(p.y-drag.origin.y))>=.45||ORTHOGONAL[drag.start].includes(Math.floor(p.y)*8+Math.floor(p.x));drag.target=dragTarget(drag.start,drag.origin,p);repaint();}
 board.addEventListener('pointerdown',e=>{
  if(drag||!enabled()||e.button!==0||!e.isPrimary)return;
  const p=point(e),c=Math.floor(p.x),r=Math.floor(p.y),start=r*8+c;
  if(c<0||c>7||r<0||r>7||state().board[start])return;
  e.preventDefault();board.querySelector(`[data-cell="${start}"]`)?.focus({preventScroll:true});
  drag={id:e.pointerId,start,origin:p,target:null,moved:false};board.setPointerCapture(e.pointerId);
 });
 board.addEventListener('pointermove',e=>{if(drag?.id!==e.pointerId)return;if(!enabled()){cancel();return;}e.preventDefault();update(e);});
 board.addEventListener('pointerup',e=>{
  if(drag?.id!==e.pointerId)return;e.preventDefault();update(e);
  const {start,target,moved}=drag,m=enabled()?previewMove(state(),start,target):null;cancel();
  if(!enabled())return;if(moved){if(m)play(m);}else select(start);
 });
 board.addEventListener('pointercancel',cancel);board.addEventListener('lostpointercapture',cancel);
 board.addEventListener('contextmenu',e=>e.preventDefault());board.addEventListener('selectstart',e=>e.preventDefault());
 board.addEventListener('keydown',e=>{
  if(!e.target.hasAttribute('data-cell')||!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Enter',' '].includes(e.key))return;
  e.preventDefault();e.stopPropagation();if(!enabled())return;
  const m=keyboardMove(state(),Number(e.target.dataset.cell),e.key);if(m)play(m);
 });
 // Assistive-technology activation retains the existing two-cell fallback.
 board.addEventListener('click',e=>{if(e.detail===0&&enabled()&&e.target.hasAttribute('data-cell'))select(Number(e.target.dataset.cell));});
 return {cancel,repaint};
}
