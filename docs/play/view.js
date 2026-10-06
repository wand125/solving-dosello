// Independent SVG presentation. Shared by the playable demo and paper figure.
const ns=document.querySelector('svg').namespaceURI;
export function svg(tag,attributes={},text='') {const el=document.createElementNS(ns,tag);for(const [k,v] of Object.entries(attributes))el.setAttribute(k,v);el.textContent=text;return el;}
export function renderBoard(target,state,{moves=[],values=new Map(),best=[],selected=null,onCell=null,onMove=null,overlay=false,lastMove=null}={}) {
 const focused=target.contains(document.activeElement)?document.activeElement.dataset.cell:null;
 target.replaceChildren();target.setAttribute('viewBox','0 0 460 460');
 const cell=50,offset=30;
 for(let i=0;i<8;i++){
  target.append(svg('text',{x:offset+25+i*cell,y:19,'text-anchor':'middle',fill:'#526469','font-size':13},'abcdefgh'[i]));
  target.append(svg('text',{x:17,y:offset+30+i*cell,'text-anchor':'middle',fill:'#526469','font-size':13},String(i+1)));
 }
 for(let i=0;i<64;i++){
  const r=svg('rect',{x:offset+(i%8)*cell,y:offset+(i>>3)*cell,width:cell,height:cell,fill:i===selected?'#eed16a':((i%8+(i>>3))%2?'#d8dade':'#e5e6e9'),stroke:'#c3d0cc','stroke-width':.6});
  r.setAttribute('data-cell',i);
  if(onCell&&!state.board[i]){r.setAttribute('role','button');r.setAttribute('tabindex','0');r.setAttribute('aria-label','abcdefgh'[i%8]+((i>>3)+1));}

  target.append(r);
 }
 for(let a=0;a<64;a++)if(state.board[a]&&state.pair[a]>a){
  const b=state.pair[a],horizontal=b-a===1,x=offset+a%8*cell+6,y=offset+(a>>3)*cell+6;
  const group=svg('g',{'pointer-events':'none'});
  group.append(svg('rect',{x,y,width:horizontal?88:38,height:horizontal?38:88,rx:7,fill:state.board[a]===1?'#292c32':'#fafafa',stroke:'#5f7476','stroke-width':1.3}));
  for(const i of [a,b])group.append(svg('circle',{cx:offset+i%8*cell+25,cy:offset+(i>>3)*cell+25,r:4,fill:state.board[a]===1?'#8ba6a6':'#b5a78c'}));
  target.append(group);
  if(lastMove=== [a,b].map(i=>'abcdefgh'[i%8]+((i>>3)+1)).join('-'))target.append(svg('rect',{x:x-2,y:y-2,width:horizontal?92:42,height:horizontal?42:92,rx:8,fill:'none',stroke:'#657dab','stroke-width':3,'pointer-events':'none'}));
 }
 for(const m of moves){
  const [a,b]=m.cells,h=b-a===1,name=m.cells.map(i=>'abcdefgh'[i%8]+((i>>3)+1)).join('-');
  const highlight=best.includes(name);
  const x=offset+a%8*cell+3,y=offset+(a>>3)*cell+3,w=h?94:44,height=h?44:94;
  const g=svg('g',{'pointer-events':onMove?'auto':'none'});
  g.append(svg('rect',{x,y,width:w,height,rx:10,fill:highlight?'#f5d65322':'#ffffff08',stroke:highlight?'#d4a900':'#949aa5','stroke-width':highlight?4:.6,'stroke-dasharray':highlight?'':'3 3'}));
  if(overlay&&values.has(name))g.append(svg('text',{class:'eval-label',x:x+w/2,y:y+height/2+4,'text-anchor':'middle',fill:'#123239','font-size':10,'font-weight':700,'paint-order':'stroke',stroke:'#f6faf7','stroke-width':3},values.get(name)));
  if(onMove){g.setAttribute('role','button');g.setAttribute('tabindex','0');g.setAttribute('aria-label',name);g.addEventListener('click',()=>onMove(m));g.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onMove(m);}});}
  target.append(g);
 }
 if(focused!==null)target.querySelector(`[data-cell="${focused}"]`)?.focus({preventScroll:true});
}
