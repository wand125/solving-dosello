#!/usr/bin/env python3
"""Deployment gate: fixed golden values plus perft, strictly small local solves."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--binary', default='./target/release/job')
    a = p.parse_args()
    binary = str(Path(a.binary).resolve())
    def run(*args):
        return json.loads(subprocess.check_output([binary, *args], text=True, timeout=30))
    assert run('--type', 'perft')['leaves'] == 49498
    with tempfile.TemporaryDirectory() as tmp:
        for i, case in enumerate(json.loads(Path(__file__).with_name('fixtures.json').read_text())):
            r = run('--type', 'exact', '--threads', '1', '--tt-mb', '4', '--time', '10000',
                    '--checkpoint', f'{tmp}/{i}.json', json.dumps(case['position']))
            assert r['complete'] and r['value'] == case['value'], (i, r)
            info=run('--type','tree-inspect',json.dumps(case['position']))
            request=dict(key=info['key'],mode='all',lower=-64,upper=64,children=[])
            tree=run('--type','tree-batch','--selectivity','0','--threads','1','--tt-mb','4','--time','10000','--requests',json.dumps([request]))
            assert tree['complete'] and tree['lower']==tree['upper']==case['value'], (i,tree)
            assert all(c['lower']==c['upper'] for c in tree['children'])
    print('self-test OK: perft, golden exact values, and tree protocol v1')


if __name__ == '__main__':
    main()
