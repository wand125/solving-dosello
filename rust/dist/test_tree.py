#!/usr/bin/env python3
"""Stage 2 integration: local fake hosts only, <=2 search threads, <=28 empties."""
import contextlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time
import unittest
from tree import Store, pack, unpack, decode, RUST, BINARY

HERE=Path(__file__).resolve().parent
BINARY=Path(os.environ.get('DOSELLO_JOB',str(BINARY)))
EXPORT=BINARY.with_name('book_export_shards')

def run(*args,**kw):
    return subprocess.run(list(map(str,args)),check=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=120,**kw)

def output(*args): return json.loads(run(*args).stdout)

class SolutionTree(unittest.TestCase):
    def fixture(self,d,empties=26,seed=1):
        root=output(BINARY,'--type','sample','--empties',empties,'--seed',seed)
        (d/'root.json').write_text(json.dumps(root))
        return [sys.executable,HERE/'tree.py','--local-test','--binary',BINARY,'--json',d/'root.json',
                '--no-seed','--state',d/'tree.sqlite','--ply','4','--batch','8','--timeout','30']

    def verify(self,d,target=4):
        store=Store(d/'tree.sqlite');self.assertTrue(store.getmeta('complete'))
        rows=store.db.execute('SELECT r.*,n.* FROM reach r JOIN nodes n USING(k)').fetchall()
        reach={(r['k'],r['color']):r['ply'] for r in rows}
        for r in rows:
            self.assertTrue(r['done']);info=decode(r['info'])
            self.assertGreaterEqual(r['quality'],1 if r['color']==info['side'] else 2)
            if info['terminal']: continue
            if r['color']==info['side']:
                children=[c for c in info['children'] if r['chosen'] in c['moves']]
                self.assertEqual(len(children),1)
            else: children=info['children']
            for c in children:
                b=store.db.execute('SELECT lo,hi FROM nodes WHERE k=?',(pack(c['key']),)).fetchone()
                self.assertEqual(b[0],b[1])
                ply=r['ply']+(c['moves']!=['pass'])
                if ply<=target: self.assertLessEqual(reach[pack(c['key']),r['color']],ply)
        store.records(d/'records.jsonl');store.snapshot(d/'snapshot.jsonl');store.db.close()
        verified=output(BINARY,'--type','tree-verify','--source',d/'records.jsonl')
        self.assertGreater(verified['verifiedParents'],0)
        return verified

    def test_both_colors_all_nodes_against_native_reference(self):
        for empties,seed,split in [(26,1,38),(28,12345,22)]:
            with self.subTest(empties=empties),tempfile.TemporaryDirectory() as tmp:
                d=Path(tmp);cmd=self.fixture(d,empties,seed)+['--analyze-empties',split]
                run(*cmd,'--simulate-failure','--stop-after','2')
                s=Store(d/'tree.sqlite')
                self.assertGreater(s.db.execute('SELECT count(*) FROM jobs WHERE error IS NOT NULL').fetchone()[0],0)
                s.db.close();run(*cmd)
                self.verify(d)
                # Re-run is idempotent; no new solver jobs.
                db=sqlite3.connect(d/'tree.sqlite');before=db.execute('SELECT count(*) FROM jobs').fetchone()[0]
                run(*cmd);self.assertEqual(db.execute('SELECT count(*) FROM jobs').fetchone()[0],before);db.close()
                print('Verified',empties,'empty both-color depth-4 tree',flush=True)

    def test_sigkill_resume_and_lease_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);cmd=self.fixture(d)
            slow=d/'slow-job'
            slow.write_text('#!/usr/bin/env python3\nimport os,sys,time\nif "tree-batch" in sys.argv: time.sleep(1)\n'+f'os.execv({str(BINARY)!r},[{str(BINARY)!r}]+sys.argv[1:])\n')
            slow.chmod(0o755);cmd[cmd.index('--binary')+1]=slow
            with open(os.devnull,'w') as log:
                p=subprocess.Popen(list(map(str,cmd)),stdout=log,stderr=log)
                try:
                    until=time.monotonic()+15;attempt=None
                    while time.monotonic()<until:
                        with contextlib.suppress(sqlite3.Error):
                            db=sqlite3.connect(d/'tree.sqlite');r=db.execute('SELECT id FROM jobs WHERE active=1').fetchone();db.close()
                            if r and (d/'dist/running'/r[0]/'pid.json').exists(): attempt=r[0];break
                        time.sleep(.03)
                    self.assertIsNotNone(attempt);p.kill();p.wait(timeout=5)
                    deadline=time.monotonic()+5
                    while (d/'dist/running'/attempt).exists() and time.monotonic()<deadline: time.sleep(.05)
                    self.assertFalse((d/'dist/running'/attempt).exists())
                    cmd[cmd.index('--binary')+1]=BINARY;run(*cmd);self.verify(d)
                finally:
                    if p.poll() is None: p.kill();p.wait()

    def test_pass_terminal_and_local_guard(self):
        for seed in [0,2]:
            with tempfile.TemporaryDirectory() as tmp:
                d=Path(tmp);cmd=self.fixture(d,8,seed);run(*cmd);self.verify(d)
        with tempfile.TemporaryDirectory() as tmp:
            p=subprocess.run([sys.executable,str(HERE/'tree.py'),'--local-test','--state',tmp+'/guard.sqlite'],capture_output=True)
            self.assertNotEqual(p.returncode,0);self.assertIn(b'<=30',p.stderr)

    def test_extend_target_without_discarding_proofs(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);cmd=self.fixture(d);cmd[cmd.index('--ply')+1]='0'
            run(*cmd);self.verify(d,0)
            s=Store(d/'tree.sqlite');before=s.db.execute('SELECT count(*) FROM nodes WHERE lo=hi').fetchone()[0];s.db.close()
            cmd[cmd.index('--ply')+1]='2';run(*cmd);self.verify(d,2)
            s=Store(d/'tree.sqlite');self.assertEqual(s.getmeta('settings')['ply'],2)
            self.assertGreaterEqual(s.db.execute('SELECT count(*) FROM nodes WHERE lo=hi').fetchone()[0],before);s.db.close()

    def test_proof_conflict_and_estimate_exclusion(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);root=output(BINARY,'--type','sample','--empties',26,'--seed',1)
            k=output(BINARY,'--type','tree-inspect',json.dumps(root))['key']
            s=Store(d/'s.sqlite');s.merge(k,2,4)
            with self.assertRaises(ValueError): s.merge(k,6,6)
            self.assertEqual(tuple(s.db.execute('SELECT lo,hi FROM nodes').fetchone()),(2,4));s.db.close()
            # A large-looking estimate with unknown interval must produce no seed.
            (d/'estimate.jsonl').write_text(json.dumps(dict(position=root,lower=-64,upper=64,estimate=64,bound='estimate'))+'\n')
            self.assertEqual(run(BINARY,'--type','tree-seed','--source',d/'estimate.jsonl').stdout,'')

if __name__=='__main__': unittest.main()
