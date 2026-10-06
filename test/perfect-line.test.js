import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {initialState, parseMove, applyMove, score, isGameOver, ORTHOGONAL} from '../docs/play/engine/rules.js';
import {perfectLine} from '../docs/perfect-line.js';

const records = readFileSync(new URL('../docs/data/perfect-line.jsonl', import.meta.url), 'utf8').trim().split('\n').map(JSON.parse);
test('full perfect line is legal, matches saved proof rows and ends 30–28', () => {
  assert.deepEqual(perfectLine, records.find(r => r.seq).seq);
  assert.equal(perfectLine.length, 25);
  let state = initialState();
  for (const [ply, move] of perfectLine.entries()) {
    assert.equal(isGameOver(state), false);
    const proof = records.find(r => r.ply === ply);
    if (ply >= 3) {
      assert.equal(proof.best, move);
      assert.equal(proof.empties, score(state).empty);
      assert.equal(proof.value, state.turn * 2);
      assert.equal(proof.exact && proof.proof, true);
    }
    state = applyMove(state, parseMove(state, move));
  }
  assert.equal(isGameOver(state), true);
  assert.deepEqual(score(state), {black: 30, white: 28, empty: 6, difference: 2});
  for (let i = 0; i < 64; i++) {
    if (!state.board[i]) assert.ok(ORTHOGONAL[i].every(j => state.board[j]));
  }
});

test('paper tables match final source counts and all perfect-line moves', () => {
  const page = readFileSync(new URL('../docs/index.html', import.meta.url), 'utf8');
  const data = JSON.parse(readFileSync(new URL('../docs/data/position-counts.json', import.meta.url), 'utf8'));
  for (const row of data.exact) {
    for (const key of ['paths', 'raw', 'd4']) assert.ok(page.includes(Number(row[key]).toLocaleString('en-US')));
  }
  for (const move of perfectLine) assert.ok(page.includes(`<td>${move}</td>`));
  assert.ok(page.includes('id="pv-last"'));
  assert.ok(page.includes('max="25"'));
  assert.doesNotMatch(page, /24\.12|28\.63|24\.43|24\.63|10²⁴/);
});
