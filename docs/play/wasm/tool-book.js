// DSTOOL01: canonical position (33 bytes), proven move (2), exact disc value (1).
import {canonical,cell} from './book-symmetry.js';
import {fromOriginal,legalMoves} from '../engine/rules.js';
const notation=a=>String.fromCharCode(97+a%8)+(1+(a>>3));
const key=(bits,side)=>bits.map(b=>b.toString(16)).join('/')+'/'+side;
export function decodeToolBook(source){
 const b=source instanceof Uint8Array?source:new Uint8Array(source),v=new DataView(b.buffer,b.byteOffset,b.byteLength);
 if(b.length<12||new TextDecoder().decode(b.subarray(0,8))!=='DSTOOL01')throw Error('Invalid tool book');
 const n=v.getUint32(8,true),rows=new Map();
 if(n>1000000||b.length!==12+n*36)throw Error('Invalid tool size');
 for(let at=12;at<b.length;at+=36){
  const bits=Array.from({length:4},(_,j)=>v.getBigUint64(at+j*8,true)),side=v.getInt8(at+32),a=b[at+33],z=b[at+34],value=v.getInt8(at+35);
  if(![1,-1].includes(side)||a>=64||z>=64||!(z-a===8||z-a===1&&(a>>3)===(z>>3))||value< -64||value>64)throw Error('Invalid tool row');
  const k=key(bits,side);if(rows.has(k))throw Error('Duplicate tool position');rows.set(k,{a,z,value});
 }
 return {size:n,lookup(position){
  const k=canonical(position),r=rows.get(key(k.bits,k.side));if(!r)return null;
  const inverse=Array(64);for(let a=0;a<64;a++)inverse[cell(a,k.transform)]=a;
  const cells=[inverse[r.a],inverse[r.z]].sort((a,b)=>a-b);
  if(!legalMoves(fromOriginal(position)).some(m=>m.cells[0]===cells[0]&&m.cells[1]===cells[1]))return null;
  const move=cells.map(notation).join('-'),m={move,cells,value:r.value,lower:r.value,upper:r.value,exact:true,depth:0,pv:[move],nodes:0,source:'tool-book'};
  return {moves:[m],bestMove:move,value:r.value,lower:r.value,upper:r.value,exact:true,complete:false,source:'tool-book',perspective:'side-to-move',elapsedMs:0,nodes:0};
 }};
}
export async function loadToolBook(url,fetcher=globalThis.fetch){const response=await fetcher(url);if(!response.ok)throw Error(`Tool book HTTP ${response.status}`);return decodeToolBook(await response.arrayBuffer());}
