#!/usr/bin/env python3
"""Cheap certificate consistency check: native rule inspection, never search or SSH."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def verify(path, binary):
    raw = Path(path).read_bytes()
    state = json.loads(raw)
    nodes = state['nodes']
    identity = lambda p: hashlib.sha256(json.dumps(p, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    bounds = lambda n: (n['lower'], n['upper'])
    evidence = {k: [-64, 64] for k in nodes}
    def merge(k, lo, hi):
        assert type(lo) is int and type(hi) is int and -64 <= lo <= hi <= 64, k
        evidence[k] = [max(evidence[k][0], lo), min(evidence[k][1], hi)]
        assert evidence[k][0] <= evidence[k][1], k
    for j in state['jobs']:
        r = j['result']; k = j['id']
        assert r['version'] == 1 and r['perspective'] == 'side-to-move'
        assert identity(r['position']) == k and r['position'] == nodes[k]['position']
        assert r['type'] == j['type'] and r['threshold'] == j['threshold']
        lo, hi = bounds(r)
        if r['complete']:
            assert lo == hi if j['type'] == 'exact' else lo >= j['threshold'] or hi < j['threshold']
        merge(k, lo, hi)
    edges = moves = unknown = 0
    for k, n in nodes.items():
        assert identity(n['position']) == k
        assert -64 <= n['lower'] <= n['upper'] <= 64
        info = json.loads(subprocess.check_output([binary, '--type', 'inspect', json.dumps(n['position'])]))
        assert info['position'] == n['position'] and info['empties'] == n['info']['empties']
        if info['terminal']:
            merge(k, info['terminalValue'], info['terminalValue'])
        if n['children']:
            # Native inspect plays EVERY legal move, then groups canonical children.
            expected = {m: identity(c['position']) for c in info['children'] for m in c['moves']}
            actual = {}
            for c in n['children']:
                assert c['id'] in nodes
                for m in c['moves']:
                    assert m not in actual
                    actual[m] = c['id']
            assert actual == expected, ('illegal/missing/symmetric edge', k)
            cs = [nodes[c['id']] for c in n['children']]
            lo, hi = max(-c['upper'] for c in cs), max(-c['lower'] for c in cs)
            assert max(lo, n['lower']) <= min(hi, n['upper']), ('negamax conflict', k)
            edges += len(cs); moves += len(actual)
        else:
            assert evidence[k] == list(bounds(n)), ('leaf lacks job evidence', k)
            unknown += evidence[k] == [-64, 64]
    # Rebuild every interval from job results + rule terminals + negamax only.
    for _ in range(len(nodes) + 1):
        before = {k: v[:] for k, v in evidence.items()}
        for k, n in nodes.items():
            if n['children']:
                cs = [evidence[c['id']] for c in n['children']]
                merge(k, max(-c[1] for c in cs), max(-c[0] for c in cs))
        if before == evidence:
            break
    for k, n in nodes.items():
        assert evidence[k] == list(bounds(n)), ('unsupported bound', k, evidence[k], bounds(n))
    root = nodes[state['root']]
    assert bounds(root) == (2, 2) and state['complete'] and state['value'] == 2
    best = sorted(m for c in root['children'] if bounds(nodes[c['id']]) == (-2, -2) for m in c['moves'])
    assert best == sorted(state['bestMoves']) == ['c5-c6', 'f3-f4']
    child = lambda n, m: nodes[next(c['id'] for c in n['children'] if m in c['moves'])]
    reply = child(root, 'f3-f4')
    table = {m: bounds(nodes[c['id']]) for c in reply['children'] for m in c['moves']}
    exact = {'e6-f6': 2, 'g4-g5': 6, 'e2-f2': 10, 'f6-g6': 10, 'f6-f7': 12, 'd7-d8': 14}
    for m, (lo, hi) in table.items():
        assert (lo, hi) == (exact[m], exact[m]) if m in exact else lo >= 14
    assert exact.keys() <= table.keys()
    assert bounds(child(child(reply, 'e6-f6'), 'd7-e7')) == (-2, -2)
    report = [f'PASS {path}', f'SHA256 {hashlib.sha256(raw).hexdigest()}',
              f'{len(nodes)} nodes; {edges} child groups; {moves} legal moves including symmetric duplicates.',
              f'{len(state["jobs"])} saved job results; {len(state.get("failures", []))} failures.',
              f'All intervals reconstructed from saved job bounds and native negamax; {unknown} unknown [-64,64] leaves.',
              'Root [2,2]; best moves c5-c6, f3-f4; PV f3-f4 e6-f6 d7-e7 verified.',
              'Replies after f3-f4 (BLACK perspective; White is choosing):']
    report += [f'  {m}: [{lo},{hi}]' for m, (lo, hi) in sorted(table.items())]
    report += ['Scope: validates certificate consistency and legal transitions; trusts saved full-depth job results, does not re-solve leaves. No network or search.']
    return '\n'.join(report) + '\n'

if __name__ == '__main__':
    if not __debug__:
        raise SystemExit('Verification requires assertions; do not use python -O')
    p = argparse.ArgumentParser()
    p.add_argument('--proof', default='proof/initial-proof.json')
    p.add_argument('--binary', default='rust/target/release/job')
    p.add_argument('--report', default='proof/verification.txt')
    a = p.parse_args()
    report = verify(a.proof, a.binary)
    Path(a.report).write_text(report)
    print(report)
