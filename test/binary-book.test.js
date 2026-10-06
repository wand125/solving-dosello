import {test} from 'node:test';
import assert from 'node:assert/strict';
import {decodeBook} from '../docs/play/wasm/dosello.js';
test('compact book is strict about versions, truncation and proof flags',()=>{
 const bytes=new Uint8Array(12+34+7),v=new DataView(bytes.buffer);bytes.set(new TextEncoder().encode('DSBOOK02'));v.setUint32(8,1,true);
 bytes[44]=1;bytes[45]=1;bytes.set([0,1,6,6,6,20,1],46);
 const entry=decodeBook(bytes).entries[0];assert.equal(entry.analysis.moves[0].value,6);assert.equal(entry.analysis.complete,true);
 assert.throws(()=>decodeBook(bytes.subarray(0,50)));const corrupt=bytes.slice();corrupt[49]=4;assert.throws(()=>decodeBook(corrupt));
 assert.throws(()=>decodeBook(new Uint8Array()));
});
