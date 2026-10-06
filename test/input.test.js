import test from 'node:test';
import assert from 'node:assert/strict';
import {dragTarget,previewMove,keyboardMove,placementEnabled} from '../docs/play/input.js';
import {initialState,legalMoves} from '../docs/play/engine/rules.js';
test('drag threshold, dominant axis, diagonal ambiguity and direct neighbor',()=>{
 const origin={x:2.5,y:2.5};
 assert.equal(dragTarget(18,origin,{x:2.9,y:2.5}),null);
 assert.equal(dragTarget(18,origin,{x:2.96,y:2.6}),19);
 assert.equal(dragTarget(18,origin,{x:2.6,y:1.8}),10);
 assert.equal(dragTarget(18,origin,{x:3.6,y:3.6}),null);
 assert.equal(dragTarget(18,origin,{x:4,y:3.7}),19);
 assert.equal(dragTarget(18,{x:2.99,y:2.1},{x:3.01,y:2.1}),19);
});
test('edge cells never wrap or leave board',()=>{
 for(const [start,origin,point] of [[0,{x:.5,y:.5},{x:-1,y:.5}],[7,{x:7.5,y:.5},{x:9,y:.5}],[0,{x:.5,y:.5},{x:.5,y:-1}],[63,{x:7.5,y:7.5},{x:7.5,y:9}]])assert.equal(dragTarget(start,origin,point),null);
 assert.equal(dragTarget(0,{x:.5,y:.5},{x:1.5,y:.5}),1);
});
test('preview requires captures, adjacency and two empty cells',()=>{
 const s=initialState();
 for(let a=0;a<64;a++)for(let b=0;b<64;b++)assert.deepEqual(previewMove(s,a,b),legalMoves(s).find(m=>m.cells.includes(a)&&m.cells.includes(b)&&a!==b)??null);
 assert.equal(previewMove(s,21,null),null);
 assert.equal(previewMove(s,0,1),null);
 assert.equal(previewMove(s,20,21),null);
});
test('keyboard uses legal directional placements; activation keys do nothing',()=>{
 const s=initialState();
 assert.deepEqual(keyboardMove(s,21,'ArrowDown')?.cells,[21,29]);
 assert.deepEqual(keyboardMove(s,29,'ArrowUp')?.cells,[21,29]);
 for(const [cell,key] of [[21,'Enter'],[21,' '],[7,'ArrowRight'],[0,'ArrowUp'],[63,'ArrowDown'],[0,'ArrowLeft'],[20,'ArrowRight'],[0,'ArrowRight']])assert.equal(keyboardMove(s,cell,key),null);
});
test('only human turns accept placement, including analysis and two players',()=>{
 for(const turn of [1,-1]){assert(placementEnabled('analysis',turn));assert(placementEnabled('two',turn));assert.equal(placementEnabled('black',turn),turn===1);assert.equal(placementEnabled('white',turn),turn===-1);assert.equal(placementEnabled('analysis',turn,true),false);}
});
