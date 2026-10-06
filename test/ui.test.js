import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {dictionaries} from '../docs/play/i18n.js';
import {resolveLanguage} from '../docs/language.js';
import {parseSequence} from '../docs/play/record.js';
import {provenBest,bestMoves} from '../docs/play/analysis.js';
test('English and Japanese cover all UI dictionary keys and HTML bindings',()=>{
 assert.deepEqual(Object.keys(dictionaries.en).sort(),Object.keys(dictionaries.ja).sort());
 for(const lang of Object.values(dictionaries))for(const value of Object.values(lang))assert(value.length);
 const html=readFileSync(new URL('../docs/play/index.html',import.meta.url),'utf8');
 for(const [,key] of html.matchAll(/data-i18n="([^"]+)"/g))assert(key in dictionaries.en,key);
});
test('language precedence is URL, saved preference, English default',()=>{
 assert.equal(resolveLanguage('',''),'en');assert.equal(resolveLanguage('','ja'),'ja');assert.equal(resolveLanguage('?lang=en','ja'),'en');assert.equal(resolveLanguage('?lang=ja','en'),'ja');assert.equal(resolveLanguage('?lang=invalid','ja'),'ja');
});
test('both pages default to English with self-only scripts and WASM-compatible CSP',()=>{
 for(const path of ['../docs/index.html','../docs/play/index.html']){
  const html=readFileSync(new URL(path,import.meta.url),'utf8');assert.match(html,/<html lang="en">/);assert.match(html,/http-equiv="Content-Security-Policy"/);assert.match(html,/script-src 'self' 'wasm-unsafe-eval'/);assert.match(html,/connect-src 'self'/);assert.doesNotMatch(html,/<script(?![^>]*\bsrc=)[^>]*>/);assert.match(html,/data-language="ja"/);
 }
});
test('sequence import is legal, canonical, atomic and preserves the full review timeline',()=>{
 const {states,moves}=parseSequence('F4-F3 e6-f6 d7-e7');assert.equal(states.length,4);assert.deepEqual(moves,['f3-f4','e6-f6','d7-e7']);assert.equal(states[0].turn,1);assert.equal(states[3].turn,-1);
 assert.throws(()=>parseSequence('f3-f4 a1-a2'),/2/);assert.throws(()=>parseSequence('pass'),/1/);assert.equal(parseSequence('').states.length,1);
});
test('recommendations cannot be confused with proven best moves',()=>{
 const moves=[{move:'a',value:2,exact:true},{move:'b',value:5},{move:'c',value:9,upper:9}];assert.deepEqual(provenBest(moves),[]);assert.deepEqual(bestMoves(moves),['b']);assert.deepEqual(provenBest([{move:'a',value:2,exact:true},{move:'b',upper:2,value:2}]),['a']);
});

test('missing move evaluations never prove an exact move best',()=>{const values=[{move:'a',exact:true,value:2},{move:'b'}];assert.deepEqual(provenBest(values),[]);assert.deepEqual(bestMoves([{move:'a',value:3},{move:'b'}]),['a']);});

import {resolveAnalysis} from '../docs/play/preferences.js';
test('analysis defaults off; explicit URL overrides saved preference',()=>{
 for(const [query,saved,expected] of [['',null,false],['','1',true],['','0',false],['?analysis=1','0',true],['?analysis=0','1',false],['?analysis=invalid','1',true]])assert.equal(resolveAnalysis(query,saved),expected);
});
test('play modes are AI black, AI white and two players',()=>{
 const html=readFileSync(new URL('../docs/play/index.html',import.meta.url),'utf8');
 assert.deepEqual([...html.match(/<select id="mode">(.*?)<\/select>/)[1].matchAll(/value="(.*?)"/g)].map(m=>m[1]),['black','white','two']);
});

test('AI time is fixed and review controls are translated without inline handlers',()=>{
 const html=readFileSync(new URL('../docs/play/index.html',import.meta.url),'utf8');
 const app=readFileSync(new URL('../docs/play/app.js',import.meta.url),'utf8');
 assert.doesNotMatch(html,/id="time"|data-i18n="time"/);assert.match(app,/timeMs:3000/);
 assert(!('time' in dictionaries.en));assert(!('time' in dictionaries.ja));
 for(const id of ['review','review-panel','review-rows','review-end','review-close'])assert(html.includes(`id="${id}"`));
 assert.doesNotMatch(html,/\sonclick=/);
});
