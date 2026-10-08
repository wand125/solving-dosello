/** Raw std-only WASM. Synchronous analyze: run in a Web Worker for responsive UI. */
export async function init(url = new URL('./dosello_ai.wasm', import.meta.url)) {
  const bytes = url instanceof ArrayBuffer || ArrayBuffer.isView(url) ? url : await (await fetch(url)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, { env: { now_ms: () => performance.now() } });
  const e = instance.exports, encoder = new TextEncoder(), decoder = new TextDecoder();
  const books = new Map(), binaryBooks = new Map();
  // Keep packed records and decode only the requested position. Large books
  // need neither 40,000 board arrays nor hundreds of thousands of move objects.
  const addBinary=source=>{
    const b=source instanceof ArrayBuffer?new Uint8Array(source):new Uint8Array(source.buffer,source.byteOffset,source.byteLength);
    const magic=new TextDecoder().decode(b.subarray(0,8)),header=magic==='DSBOOK03'?37:34;
    if(b.length<12||!['DSBOOK02','DSBOOK03'].includes(magic))throw new Error('Invalid binary book');
    const v=new DataView(b.buffer,b.byteOffset,b.byteLength),count=v.getUint32(8,true);let at=12;
    if(count>100000)throw new Error('Book count exceeds limit');
    const pending=[];
    for(let i=0;i<count;i++){
      if(at+header>b.length)throw new Error('Truncated book');
      const start=at,side=v.getInt8(at+32),n=b[at+33];
      if(![1,-1].includes(side)||n>112||at+header+n*7>b.length)throw new Error('Invalid book record');
      const k=Array.from({length:4},(_,j)=>v.getBigUint64(at+j*8,true).toString(16)).join('/')+'/'+side;
      at+=header+n*7;pending.push([k,{bytes:b,start,end:at,magic}]);
    }
    if(at!==b.length)throw new Error('Trailing binary book data');
    for(const [k,r] of pending)binaryBooks.set(k,r);
    return binaryBooks.size+books.size;
  };
  const packedKey=p=>{
    if(typeof p==='string')return p.trim()===''?'80810100000/300c000000/1004000000/800100000/'+1:null;
    const bits=[0n,0n,0n,0n];
    for(let a=0;a<64;a++){const r=a>>3,c=a%8,m=1n<<BigInt(a);if(p.board[r][c]===1)bits[0]|=m;else if(p.board[r][c]===-1)bits[1]|=m;if(p.shape[r][c]==='h-left')bits[2]|=m;else if(p.shape[r][c]==='v-top')bits[3]|=m;}
    return bits.map(b=>b.toString(16)).join('/')+'/'+(p.turn??1);
  };
  const unpack=r=>{const b=new Uint8Array(12+r.end-r.start);b.set(new TextEncoder().encode(r.magic));new DataView(b.buffer).setUint32(8,1,true);b.set(r.bytes.subarray(r.start,r.end),12);return decodeBook(b).entries[0].analysis;};
  const key = p => typeof p === 'string' ? `moves:${p.trim()}` : JSON.stringify([p.board,p.shape,p.turn??1]);
  const api = {
    analyze(position = '', { timeMs = 1000, exactIfPossible = true, bestOnly = false, reviewSolve = false, valueOnly = false, ttMb = 32 } = {}) {
      const book = api.getBook(position);
      if (!reviewSolve && book && book.moves.length && (book.complete === true && book.moves.every(m=>m.exact && Number.isInteger(m.value)) || bestOnly && book.exact && book.moves.some(m=>m.exact && m.value===book.value))) {
        const moves=structuredClone(book.moves).map(m=>({
          ...m,
          cells:m.cells??m.move.split('-').map(c=>(Number(c[1])-1)*8+c.charCodeAt(0)-97),
          bestReply:m.bestReply??m.pv?.[1]??null,
        })).sort((a,b)=>a.cells[0]*64+a.cells[1]-b.cells[0]*64-b.cells[1]);
        const best=moves.find(m=>book.exact&&m.exact&&m.value===book.value)??moves.reduce((a,b)=>b.value>a.value?b:a);
        return {terminal:false,pass:false,value:null,perspective:'side-to-move',
          ...structuredClone(book),moves,bestMove:best.move,source:'book',elapsedMs:0,nodes:0,nps:0};
      }
      const input=encoder.encode(JSON.stringify({position,options:{timeMs,exactIfPossible,bestOnly,reviewSolve,valueOnly,ttMb}}));
      const ptr=e.alloc(input.length);
      try {
        new Uint8Array(e.memory.buffer,ptr,input.length).set(input);
        e.analyze_json(ptr,input.length);
        const result=JSON.parse(decoder.decode(new Uint8Array(e.memory.buffer,e.result_ptr(),e.result_len())));
        if(result.error)throw new Error(result.error);
        return result;
      } finally {e.free(ptr,input.length);}
    },
    getBook(position) {
      let book=books.get(key(position));
      if(!book){const record=binaryBooks.get(packedKey(position));if(record)book=unpack(record);}
      if(!book || !Array.isArray(book.moves))return null;
      return {...structuredClone(book),source:'book',moves:book.moves.filter(m=>Number.isFinite(m.value)).map(m=>({
        ...structuredClone(m),source:'book',cells:m.cells??m.move.split('-').map(c=>(Number(c[1])-1)*8+c.charCodeAt(0)-97),
        bestReply:m.bestReply??m.pv?.[1]??null,
      }))};
    },
    async loadEval3(source) {
      let bytes;
      if(source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
        bytes=source instanceof ArrayBuffer?new Uint8Array(source):new Uint8Array(source.buffer,source.byteOffset,source.byteLength);
      } else {
        const response=await fetch(source);
        if(!response.ok)throw new Error(`Evaluation HTTP ${response.status}`);
        bytes=new Uint8Array(await response.arrayBuffer());
      }
      const ptr=e.alloc(bytes.length);
      try {
        new Uint8Array(e.memory.buffer,ptr,bytes.length).set(bytes);
        if(e.load_eval3(ptr,bytes.length)!==0)throw new Error('Invalid eval3 weights');
      } finally {e.free(ptr,bytes.length);}
    },
    async loadBook(source) {
      let book;
      if(source instanceof ArrayBuffer || ArrayBuffer.isView(source)) return addBinary(source);
      else if(typeof source==='object'&&!(source instanceof URL)) book=source;
      else {const response=await fetch(source);if(!response.ok)throw new Error(`Book HTTP ${response.status}`);const bytes=await response.arrayBuffer();if(new TextDecoder().decode(new Uint8Array(bytes,0,Math.min(8,bytes.byteLength))).startsWith('DSBOOK0'))return addBinary(bytes);book=JSON.parse(new TextDecoder().decode(bytes));}
      if(book.version!==1||!Array.isArray(book.entries))throw new Error('Invalid book');
      for(const entry of book.entries){if(entry.mode==='wld')continue;books.set(key(entry.position),entry.analysis);if(typeof entry.sequence==='string')books.set(key(entry.sequence),entry.analysis);}
      return books.size;
    }
  };
  return api;
}
export default init;

/** Strict portable little-endian compact book decoder; JSON format stays supported. */
export function decodeBook(source) {
 const bytes=source instanceof ArrayBuffer?new Uint8Array(source):new Uint8Array(source.buffer,source.byteOffset,source.byteLength);
 const magic=new TextDecoder().decode(bytes.subarray(0,8)),header=magic==='DSBOOK03'?37:34;
 if(bytes.length<12||!['DSBOOK02','DSBOOK03'].includes(magic))throw new Error('Invalid binary book');
 const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);let at=12;
 const count=view.getUint32(8,true);if(count>100000)throw new Error('Book count exceeds limit');
 const entries=[];const notation=a=>String.fromCharCode(97+a%8)+(1+(a>>3));
 for(let i=0;i<count;i++) {
   if(at+header>bytes.length)throw new Error('Truncated book');
   const bits=Array.from({length:4},(_,j)=>view.getBigUint64(at+j*8,true));at+=32;
   const turn=view.getInt8(at++),n=bytes[at++];if(![1,-1].includes(turn)||n>112||at+n*7>bytes.length)throw new Error('Invalid book record');
   const proof=magic==='DSBOOK03'?{lower:view.getInt8(at++),upper:view.getInt8(at++),value:view.getInt8(at++)}:null;
   if(proof&&(proof.lower< -64||proof.upper>64||proof.lower>proof.upper||proof.value<proof.lower||proof.value>proof.upper))throw new Error('Invalid position proof');
   const board=Array.from({length:8},()=>Array(8).fill(0)),shape=Array.from({length:8},()=>Array(8).fill(''));
   for(let a=0;a<64;a++) {const mask=1n<<BigInt(a);board[a>>3][a%8]=bits[0]&mask?1:bits[1]&mask?-1:0;
     shape[a>>3][a%8]=bits[2]&mask?'h-left':bits[2]<<1n&mask?'h-right':bits[3]&mask?'v-top':bits[3]<<8n&mask?'v-bottom':'';}
   const moves=[];
   for(let j=0;j<n;j++){const a=bytes[at++],b=bytes[at++],value=view.getInt8(at++),lower=view.getInt8(at++),upper=view.getInt8(at++),depth=bytes[at++],flag=bytes[at++];
     if(a>=64||b>=64||!(b-a===8||b-a===1&&(a>>3)===(b>>3))||lower>upper||lower< -64||upper>64||value< -64||value>64||flag>1||flag===1&&(lower!==upper||value!==lower))throw new Error('Invalid book move');
     const move=notation(a)+'-'+notation(b),exact=flag===1; moves.push({move,cells:[a,b],value,lower,upper,exact,depth,bound:exact?'exact':lower> -64?'lower':upper<64?'upper':'heuristic',pv:[move],nodes:0,source:'book'});}
   const position={board,shape,turn};
   const entry={position,analysis:{moves,complete:moves.length>0&&moves.every(m=>m.exact),perspective:'side-to-move',...(proof?{...proof,exact:proof.lower===proof.upper,bound:proof.lower===proof.upper?'exact':proof.lower>-64?'lower':proof.upper<64?'upper':'heuristic'}:{})}};
   if(bits[0]===((1n<<20n)|(1n<<28n)|(1n<<35n)|(1n<<43n))&&bits[1]===((1n<<26n)|(1n<<27n)|(1n<<36n)|(1n<<37n))&&turn===1&&bits[2]===((1n<<26n)|(1n<<36n))&&bits[3]===((1n<<20n)|(1n<<35n)))entry.sequence='';
   entries.push(entry);
 }
 if(at!==bytes.length)throw new Error('Trailing binary book data');
 return {version:1,entries};
}
