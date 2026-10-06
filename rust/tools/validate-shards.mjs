// Independent archive integrity checker; no browser modules or original assets.
import {readFile,readdir} from 'node:fs/promises';
import {resolve} from 'node:path';
import assert from 'node:assert/strict';
import {fromOriginal,toOriginal,legalMoves,formatMove} from '../../engine/rules.js';
const dir=resolve(process.argv[2]),index=JSON.parse(await readFile(dir+'/index.json'));
const records=new Map();let bytes=0;
const key=p=>[p.black,p.white,p.hleft,p.vtop,p.turn].join('/');
function packed(s){const bits=[0n,0n,0n,0n];for(let i=0;i<64;i++){const b=1n<<BigInt(i);if(s.board[i]===1)bits[0]|=b;if(s.board[i]===-1)bits[1]|=b;if(s.pair[i]===i+1)bits[2]|=b;if(s.pair[i]===i+8)bits[3]|=b;}return {black:bits[0],white:bits[1],hleft:bits[2],vtop:bits[3],turn:s.turn};}
for(const prefix of await readdir(dir))if(/^[0-9a-f]{2}$/.test(prefix))for(const name of await readdir(dir+'/'+prefix))if(/^[0-9a-f]{2}\.bin$/.test(name)){
 const b=await readFile(`${dir}/${prefix}/${name}`);bytes+=b.length;assert.equal(b.subarray(0,8).toString(),'DSSHRD01');const count=b.readUInt32LE(8);assert.equal(b.readUInt32LE(12),16+4*count);assert.equal(b.readUInt32LE(12+4*count),b.length);
 let prev=-1n;for(let i=0;i<count;i++){
  const at=b.readUInt32LE(12+4*i),end=b.readUInt32LE(16+4*i);assert(end>=at+47&&end<=b.length);const hash=b.readBigUInt64LE(at);assert(hash>=prev);prev=hash;assert.equal((hash>>48n).toString(16).padStart(4,'0'),prefix+name.slice(0,2));
  const p={black:b.readBigUInt64LE(at+8),white:b.readBigUInt64LE(at+16),hleft:b.readBigUInt64LE(at+24),vtop:b.readBigUInt64LE(at+32),turn:b.readInt8(at+40)};
  const n=b[at+41],lo=b.readInt8(at+42),hi=b.readInt8(at+43);assert([-1,1].includes(p.turn));assert(lo>=-64&&lo<=hi&&hi<=64);assert.equal(end-at,47+4*n);
  const moves=[];for(let o=at+47;o<end;o+=4){const a=b[o]&63,z=a+(b[o]&64?8:1),x=b.readInt8(o+1),y=b.readInt8(o+2),kind=b[o+3]&7,depth=b[o+3]>>3;assert(z<64&&kind<=4);const lower=kind===0||kind===1||kind===3?x:-64,upper=kind===0||kind===2?x:kind===3?y:64;assert(lower<=upper);moves.push({cells:[a,z],lower,upper,exact:kind===0,depth,value:x});}
  assert(!records.has(key(p)));records.set(key(p),moves);
 }
}
assert.equal(bytes,index.bytes);assert.equal(records.size,index.records);
const transform=(i,t)=>{let r=i>>3,c=i%8;if(t>=4)c=7-c;for(let k=0;k<t%4;k++)[r,c]=[c,7-r];return r*8+c;};
function canonical(s){let best=null;for(let t=0;t<8;t++){const q={board:new Int8Array(64),pair:new Int8Array(64).fill(-1),turn:s.turn};for(let i=0;i<64;i++){q.board[transform(i,t)]=s.board[i];if(s.pair[i]>=0)q.pair[transform(i,t)]=transform(s.pair[i],t);}const p=packed(q),tuple=[p.black,p.white,p.hleft,p.vtop];if(!best||tuple.some((v,i)=>v<best.tuple[i]&&tuple.slice(0,i).every((x,j)=>x===best.tuple[j])))best={p,tuple,t,q};}return best;}
const samples=JSON.parse(await readFile(dir+'/samples.json'));let checks=0;
for(const sample of samples){const c=canonical(fromOriginal(sample.position)),actual=records.get(key(c.p));assert(actual);assert.deepEqual(actual.map(m=>m.cells.join(',')).sort(),legalMoves(c.q).map(m=>m.cells.join(',')).sort());
 for(const m of sample.analysis.moves){const cells=m.cells.map(i=>transform(i,c.t)).sort((a,b)=>a-b),got=actual.find(g=>g.cells.join(',')===cells.join(','));assert(got);assert.equal(got.lower,m.lower);assert.equal(got.upper,m.upper);assert.equal(got.exact,m.exact);if(m.depth>=index.minDepth||m.bound!=='heuristic')assert.equal(got.depth,Math.min(31,m.depth));if(m.exact)assert.equal(got.value,m.value);checks++;}}
console.log(`PASS ${records.size} shard records, ${checks} sample move intervals`);
