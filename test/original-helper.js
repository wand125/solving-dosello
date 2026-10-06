import vm from 'node:vm';
// Test-only adapter: extract pure functions without executing original UI startup.
import {mkdir,readFile,writeFile} from 'node:fs/promises';
const cache=new URL('../.cache/original/game.js',import.meta.url);
let source=null;
try { source=await readFile(cache,'utf8'); } catch {}
if(!source && process.env.DOSELLO_OFFLINE!=='1') {
  try {
    const response=await fetch('https://game2.raku-watanabe.com/game.js',{signal:AbortSignal.timeout(8000)});
    if(!response.ok)throw Error('Download unavailable');
    source=await response.text();
    await mkdir(new URL('.',cache),{recursive:true});await writeFile(cache,source);
  } catch { /* Optional external oracle; never required for offline tests. */ }
}
function extract(source){
const names=['captured','partner','candidate','moves','apply','count','nextShape','heuristic','search','choose','markPair'];
const functions=names.map(name=>{
  const start=source.indexOf('function '+name+'(');
  if(start<0)throw Error('Original function missing: '+name);
  let braces=0,end=source.indexOf('{',start);
  do {if(source[end]==='{')braces++;if(source[end]==='}')braces--;end++;}while(braces);
  return source.slice(start,end);
}).join('\n');
const constants=source.slice(source.indexOf('const N='),source.indexOf('\nconst boardEl'));
const helpers=source.slice(source.indexOf('const inside='),source.indexOf('function captured'));
const context=vm.createContext({});
vm.runInContext(`${constants}\nlet shape,strength;\n${helpers}\n${functions}\nthis.api={${names.join(',')},inside,key,set:(s,n)=>{shape=s;strength=n;},random:r=>Math.random=r};`,context);
return context.api;
}
export const original=source?extract(source):null;
export function originalMove(m){return {cells:m.cells.map(i=>[i>>3,i&7]),flips:m.flips.map(i=>[i>>3,i&7])};}
