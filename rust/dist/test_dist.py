#!/usr/bin/env python3
"""Integration tests: no SSH, max two solver threads, <=30 empties."""
import asyncio
import importlib.util
import json
import os
import signal
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent
BINARY = Path(os.environ.get('DOSELLO_JOB', str(HERE.parent/'target/release/job'))).resolve()
SOLVE = BINARY.with_name('solve')


def output(*args):
    return json.loads(subprocess.check_output(list(map(str,args)), text=True, timeout=30, stderr=subprocess.DEVNULL))


class Distributed(unittest.TestCase):
    def test_proof_restart_failure_and_all_best_moves(self):
        for empties, seed in [(26, 1), (28, 12345), (30, 91348)]:
            with self.subTest(empties=empties), tempfile.TemporaryDirectory() as tmp:
                d = Path(tmp)
                p = output(BINARY, '--type', 'sample', '--empties', empties, '--seed', seed)
                (d/'position.json').write_text(json.dumps(p))
                cmd = [sys.executable, str(HERE/'coordinator.py'), '--local-test', '--binary', str(BINARY),
                       '--json', str(d/'position.json'), '--state', str(d/'state.json'),
                       '--proof', str(d/'proof.json'), '--timeout', '20', '--split-empties', '22' if empties == 26 else '28', '--split-depth', '2']
                subprocess.run(cmd+['--simulate-failure','--stop-after','2'], check=True, timeout=45, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                before = json.loads((d/'state.json').read_text())
                self.assertTrue(before['failures'])
                self.assertTrue(before['jobs'])
                subprocess.run(cmd, check=True, timeout=60, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                proof = json.loads((d/'proof.json').read_text())
                self.assertTrue(proof['complete'])
                self.assertGreaterEqual(len(proof['jobs']), len(before['jobs']))
                ref = output(SOLVE, '--prove-best', '--threads', '1', '--tt-mb', '16', '--time', '20000', json.dumps(p))
                self.assertEqual(proof['value'], ref['value'])
                self.assertIn(ref['bestMove'], proof['bestMoves'])
                best = []
                for edge in output(BINARY, '--type', 'inspect', json.dumps(p))['children']:
                    r = output(SOLVE, '--root-score', '--threads', '1', '--tt-mb', '4', '--time', '10000', json.dumps(edge['position']))
                    self.assertTrue(r['exact'])
                    if -r['value'] == proof['value']:
                        best += edge['moves']
                self.assertEqual(sorted(best), sorted(proof['bestMoves']))
                for n in proof['nodes'].values():
                    self.assertLessEqual(n['lower'], n['upper'])
                    if n['children']:
                        cs = [proof['nodes'][c['id']] for c in n['children']]
                        self.assertGreaterEqual(n['lower'], max(-c['upper'] for c in cs))
                        self.assertLessEqual(n['upper'], max(-c['lower'] for c in cs))

    def test_job_timeout_preserves_retry_checkpoint(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = output(BINARY, '--type', 'sample', '--empties', 26, '--seed', 7128)
            args = [BINARY, '--type', 'exact', '--threads', 1, '--tt-mb', 4,
                    '--checkpoint', tmp+'/job.json', json.dumps(p)]
            r = output(*args, '--time', 0)
            self.assertFalse(r['complete'])
            self.assertTrue(Path(tmp+'/job.json').exists())
            r = output(*args, '--time', 10000)
            self.assertTrue(r['complete'])
            resumed = output(*args, '--time', 0)
            self.assertTrue(resumed['complete'])
            self.assertEqual(resumed['nodes'], 0)

    def test_pass_and_early_terminal(self):
        for seed in [0, 2]:
            with self.subTest(seed=seed), tempfile.TemporaryDirectory() as tmp:
                d = Path(tmp)
                p = output(BINARY, '--type', 'sample', '--empties', 8, '--seed', seed)
                (d/'p.json').write_text(json.dumps(p))
                subprocess.run([sys.executable, str(HERE/'coordinator.py'), '--local-test', '--binary', str(BINARY),
                                '--json', str(d/'p.json'), '--state', str(d/'s.json'), '--proof', str(d/'proof.json'),
                                '--split-empties', '4'], check=True, timeout=30, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                proof = json.loads((d/'proof.json').read_text())
                ref = output(SOLVE, '--prove-best', '--threads', 1, '--tt-mb', 4, '--time', 10000, json.dumps(p))
                self.assertEqual(proof['value'], ref['value'])
                self.assertEqual(proof['bestMoves'], [] if seed == 0 else ['pass'])

    def test_supervisor_eof_timeout_cancel(self):
        for mode in ['eof','timeout','cancel','hup']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as tmp:
                attempt = 'a'*32
                cmd = [sys.executable, str(HERE/'run_host.py'), '--timeout', '.3' if mode=='timeout' else '10',
                       attempt, '--', sys.executable, '-c', 'import time; time.sleep(30)']
                proc = subprocess.Popen(cmd, cwd=tmp, stdin=subprocess.PIPE)
                directory = Path(tmp)/'dist/running'/attempt
                try:
                    until = time.monotonic()+3
                    while not (directory/'pid.json').exists() and time.monotonic()<until:
                        time.sleep(.02)
                    pid = json.loads((directory/'pid.json').read_text())['pid']
                    if mode == 'eof':
                        proc.stdin.close()
                    elif mode == 'hup':
                        os.kill(proc.pid, signal.SIGHUP)
                    elif mode == 'cancel':
                        subprocess.run([sys.executable,str(HERE/'run_host.py'),'--cancel',attempt],cwd=tmp,check=True)
                    proc.wait(timeout=5)
                    self.assertFalse(directory.exists())
                    with self.assertRaises(ProcessLookupError):
                        os.kill(pid,0)
                finally:
                    if proc.stdin and not proc.stdin.closed:
                        proc.stdin.close()
                    if proc.poll() is None:
                        proc.kill()
                        proc.wait()

    def test_killed_coordinator_and_restart(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            # Force a predictable in-flight interval without doing extra search.
            fake = d/'slow-job'
            fake.write_text('#!/usr/bin/env python3\nimport os,sys,time\n'
                            'if "inspect" not in sys.argv: time.sleep(1)\n'
                            f'os.execv({str(BINARY)!r}, [{str(BINARY)!r}]+sys.argv[1:])\n')
            fake.chmod(0o755)
            p = output(BINARY,'--type','sample','--empties','26','--seed','44123')
            (d/'p.json').write_text(json.dumps(p))
            cmd = [sys.executable,str(HERE/'coordinator.py'),'--local-test','--binary',str(fake),
                   '--json',str(d/'p.json'),'--state',str(d/'s.json'),'--proof',str(d/'proof.json'),'--timeout','15']
            with open(os.devnull,'w') as log:
                proc = subprocess.Popen(cmd, stdout=log, stderr=log)
                try:
                    deadline = time.monotonic()+10
                    state = {}
                    while time.monotonic()<deadline:
                        if (d/'s.json').exists():
                            state = json.loads((d/'s.json').read_text())
                            if state['running']:
                                break
                        time.sleep(.03)
                    self.assertTrue(state.get('running'))
                    time.sleep(.2)
                    proc.kill()
                    proc.wait(timeout=5)
                    time.sleep(.5)
                    for j in state['running'].values():
                        self.assertFalse((HERE/'running'/j['attempt']).exists())
                    cmd[cmd.index('--binary')+1] = str(BINARY)
                    subprocess.run(cmd,check=True,timeout=45,stdout=log,stderr=log)
                    proof=json.loads((d/'proof.json').read_text())
                    ref=output(SOLVE,'--prove-best','--threads','1','--tt-mb','4','--time','10000',json.dumps(p))
                    self.assertEqual(proof['value'],ref['value'])
                finally:
                    if proc.poll() is None:
                        proc.kill()
                        proc.wait()

    def test_dry_run_and_large_local_rejection(self):
        with tempfile.TemporaryDirectory() as tmp:
            cmd=[sys.executable,str(HERE/'coordinator.py'),'--binary',str(BINARY),'--state',tmp+'/state.json']
            subprocess.run(cmd+['--dry-run', '--hosts', str(HERE/'hosts.example.json')],check=True,stdout=subprocess.DEVNULL)
            self.assertFalse(Path(tmp+'/state.json').exists())
            r=subprocess.run(cmd+['--local-test'],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
            self.assertNotEqual(r.returncode,0)
            self.assertIn(b'<=34',r.stderr)


if __name__ == '__main__':
    unittest.main()
