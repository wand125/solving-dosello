import {languageController} from './language.js';
const nodes=[...document.querySelectorAll('[data-ja]')].map(el=>({el,en:el.innerHTML,ja:el.dataset.ja}));
const labels=[...document.querySelectorAll('[data-ja-label]')].map(el=>({el,en:el.getAttribute('aria-label'),ja:el.dataset.jaLabel}));
languageController(lang=>{
 for(const {el,en,ja} of nodes)el.innerHTML=lang==='ja'?ja:en;
 for(const {el,en,ja} of labels)el.setAttribute('aria-label',lang==='ja'?ja:en);
 document.title=lang==='ja'?'DOSELLOの初期局面を解く':'DOSELLO is Solved';
 document.querySelector('meta[name=description]').content=lang==='ja'?'DOSELLOの初期局面は黒番+2。分散探索、証明木、独自エンジンと対局ツール。':'DOSELLO is solved: Black +2. Distributed search, a saved certificate and an independent playable engine.';
 document.dispatchEvent(new Event('languagechange'));
});
