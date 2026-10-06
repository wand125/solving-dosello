import {initialState, parseMove, applyMove, score, isGameOver} from './play/engine/rules.js';
import {renderBoard} from './play/view.js';
import {perfectLine} from './perfect-line.js';

const states = [initialState()];
for (const move of perfectLine) {
  const state = states.at(-1);
  states.push(applyMove(state, parseMove(state, move)));
}
const final = score(states.at(-1));
if (!isGameOver(states.at(-1)) || final.black !== 30 || final.white !== 28) {
  throw new Error('Perfect-play line has an unexpected result');
}
const input = document.getElementById('pv-step');
input.max = perfectLine.length;
function draw() {
  const i = Number(input.value), ja = document.documentElement.lang === 'ja';
  const counts = score(states[i]);
  renderBoard(document.getElementById('pv-board'), states[i]);
  const heading = i === 0 ? (ja ? '初期局面' : 'Initial position')
    : (ja ? `${i}手目：${perfectLine[i - 1]}` : `Ply ${i}: ${perfectLine[i - 1]}`);
  const tally = ja ? `黒 ${counts.black}・白 ${counts.white}・空き ${counts.empty}`
    : `Black ${counts.black} · White ${counts.white} · ${counts.empty} empty`;
  const ending = i === perfectLine.length ? (ja ? ' · 終局：黒の2石勝ち（孤立空き6）' : ' · Terminal: Black wins by 2 (6 isolated empty cells)') : '';
  document.getElementById('pv-caption').textContent = `${heading} · ${tally}${ending}`;
  for (const id of ['first', 'prev']) document.getElementById(`pv-${id}`).disabled = i === 0;
  for (const id of ['next', 'last']) document.getElementById(`pv-${id}`).disabled = i === perfectLine.length;
}
function jump(i) {
  input.value = Math.max(0, Math.min(perfectLine.length, i));
  draw();
}
input.addEventListener('input', draw);
document.getElementById('pv-first').onclick = () => jump(0);
document.getElementById('pv-prev').onclick = () => jump(+input.value - 1);
document.getElementById('pv-next').onclick = () => jump(+input.value + 1);
document.getElementById('pv-last').onclick = () => jump(perfectLine.length);
document.addEventListener('languagechange', draw);
draw();
