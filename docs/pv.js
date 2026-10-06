import {initialState,parseMove,applyMove} from './play/engine/rules.js';
import {renderBoard} from './play/view.js';
const states=[initialState()],pv=['f3-f4','e6-f6','d7-e7'];for(const m of pv)states.push(applyMove(states.at(-1),parseMove(states.at(-1),m)));
const input=document.getElementById('pv-step');function draw(){const i=Number(input.value);renderBoard(document.getElementById('pv-board'),states[i]);document.getElementById('pv-caption').textContent=i===0?(document.documentElement.lang==='ja'?'初期局面 · 黒番 · 値 +2':'Initial position · Black to move · value +2'):(document.documentElement.lang==='ja'?`${i}手目まで：`:`After ply ${i}: `)+pv.slice(0,i).join(' → ');}
input.addEventListener('input',draw);document.getElementById('pv-prev').onclick=()=>{input.value=Math.max(0,+input.value-1);draw();};document.getElementById('pv-next').onclick=()=>{input.value=Math.min(3,+input.value+1);draw();};draw();

document.addEventListener("languagechange",draw);
