// Independent SVG presentation. Shared by the playable demo and paper figure.
const ns=document.querySelector('svg').namespaceURI;
export function svg(tag,attributes={},text='') {const el=document.createElementNS(ns,tag);for(const [k,v] of Object.entries(attributes))el.setAttribute(k,v);el.textContent=text;return el;}
export function renderBoard(target,state,{moves=[],values=new Map(),best=[],selected=null,onCell=null,onMove=null,overlay=false,lastMove=null}={}) {
 const focused=target.contains(document.activeElement)?document.activeElement.dataset.cell:null;
 target.replaceChildren();target.setAttribute('viewBox','0 0 460 460');
 const cell=50,offset=30;
 for(let i=0;i<8;i++){
  target.append(svg('text',{x:offset+25+i*cell,y:19,'text-anchor':'middle',fill:'#000','font-size':13},'abcdefgh'[i]));
  target.append(svg('text',{x:17,y:offset+30+i*cell,'text-anchor':'middle',fill:'#000','font-size':13},String(i+1)));
 }
 for(let i=0;i<64;i++){
  const r=svg('rect',{x:offset+(i%8)*cell,y:offset+(i>>3)*cell,width:cell,height:cell,fill:i===selected?'#eed16a':'#1b7a3a',stroke:'#000','stroke-width':1});
  r.setAttribute('data-cell',i);
  if(onCell&&!state.board[i]){r.setAttribute('role','button');r.setAttribute('tabindex','0');r.setAttribute('aria-label','abcdefgh'[i%8]+((i>>3)+1));}

  target.append(r);
 }
 for(let a=0;a<64;a++)if(state.board[a]&&state.pair[a]>a){
  const b=state.pair[a],horizontal=b-a===1,x=offset+a%8*cell+6,y=offset+(a>>3)*cell+6;
  const group=svg('g',{'pointer-events':'none'});
  group.append(svg('rect',{x,y,width:horizontal?88:38,height:horizontal?38:88,rx:7,fill:state.board[a]===1?'#000':'#fff',stroke:'#000','stroke-width':1.3}));
  for(const i of [a,b])group.append(svg('circle',{cx:offset+i%8*cell+25,cy:offset+(i>>3)*cell+25,r:4,fill:'#1b7a3a'}));
  target.append(group);
  if(lastMove=== [a,b].map(i=>'abcdefgh'[i%8]+((i>>3)+1)).join('-'))target.append(svg('rect',{x:x-2,y:y-2,width:horizontal?92:42,height:horizontal?42:92,rx:8,fill:'none',stroke:state.board[a]===1?'#fff':'#000','stroke-width':3,'pointer-events':'none'}));
 }
 for(const m of moves){
  const [a,b]=m.cells,h=b-a===1,name=m.cells.map(i=>'abcdefgh'[i%8]+((i>>3)+1)).join('-');
  const highlight=best.includes(name);
  const x=offset+a%8*cell+3,y=offset+(a>>3)*cell+3,w=h?94:44,height=h?44:94;
  const g=svg('g',{'pointer-events':onMove?'auto':'none'});
  g.append(svg('rect',{x,y,width:w,height,rx:10,fill:'transparent',stroke:highlight?'#ffd400':'#fff','stroke-width':highlight?4:1.5,'stroke-dasharray':highlight?'':'3 3'}));
  if(overlay&&values.has(name))g.append(svg('text',{class:highlight?'eval-label best':'eval-label',x:x+w/2,y:y+height/2+4,'text-anchor':'middle',fill:'#000','font-size':10,'font-weight':700,'paint-order':'stroke',stroke:'#fff','stroke-width':3},values.get(name)));
  if(onMove){g.setAttribute('role','button');g.setAttribute('tabindex','0');g.setAttribute('aria-label',name);g.addEventListener('click',()=>onMove(m));g.addEventListener('keydown',e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();onMove(m);}});}
  target.append(g);
 }
 if(focused!==null)target.querySelector(`[data-cell="${focused}"]`)?.focus({preventScroll:true});
}
