#!/usr/bin/env python3
"""Read-only distributed proof status (no SSH or solver calls)."""
import argparse
import json
from pathlib import Path
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--state', default=str(Path(__file__).with_name('state.json')))
a = p.parse_args()
s = json.loads(Path(a.state).read_text())
n = s['nodes'][s['root']]
print(f"complete={s['complete']} root=[{n['lower']},{n['upper']}] jobs={len(s['jobs'])} outstanding={len(s.get('pending', {}))} failures={len(s.get('failures', []))}")
for host, j in s['running'].items():
    print(f"{host}: {j['type']} {j['id'][:12]} {j['empties']} empties, {time.time()-j['started']:.0f}s")
remaining = 0
for edge in n['children']:
    c = s['nodes'][edge['id']]
    lo, hi = -c['upper'], -c['lower']
    remaining += lo != hi
    print(f"{','.join(edge['moves'])}: [{lo},{hi}]")
completed = [j['elapsed'] for j in s['jobs'] if j.get('result', {}).get('complete')]
if s['complete']:
    print('ETA: 0s (proof complete)')
elif completed:
    print(f"Mean completed leaf time: {sum(completed)/len(completed):.2f}s")
    print(f"ETA proxy: {remaining*sum(completed)/len(completed):.0f}s (unresolved root groups × mean leaf time; excludes future splits/ties; NOT a completion forecast)")
else:
    print('ETA unavailable: no completed jobs')
if s['complete']:
    print(f"value={s['value']} bestMoves={s['bestMoves']}")
