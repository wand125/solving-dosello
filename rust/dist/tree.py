#!/usr/bin/env python3
"""BFS exact solution trees, both AI colors. SQLite WAL is the durable journal/index.
Ply counts placements; a forced pass stays at the same ply. Only one certified
AI tie is followed; every tie is recorded. Boundary opponent children have exact
values; boundary AI nodes have all ties exact and all other moves strictly bounded.
No remote work is started by --estimate, --dry-run, --snapshot or --local-test.
"""
import argparse
import asyncio
import contextlib
import fcntl
import json
import os
from pathlib import Path
import shlex
import signal
import sqlite3
import subprocess
import sys
import time
import uuid
import zlib
from coordinator import HERE, RUST, SSH, JobQueue, atomic, remote_cd

BINARY = RUST/'target/release/job'

def pack(k):
    if len(k) != 65 or k[-1] not in 'bw':
        raise ValueError('invalid tree key')
    return bytes.fromhex(k[:64])+k[-1].encode()

def unpack(k):
    return k[:32].hex()+chr(k[32])

def encode(x):
    return zlib.compress(json.dumps(x, separators=(',', ':')).encode())

def decode(x):
    return json.loads(zlib.decompress(x))

def bounds(row):
    lo, hi = row['lower'], row['upper']
    if type(lo) is not int or type(hi) is not int or not -64 <= lo <= hi <= 64:
        raise ValueError('invalid proof bounds')
    return lo, hi

class Store:
    def __init__(self, path):
        self.db = sqlite3.connect(path)
        self.db.row_factory = sqlite3.Row
        self.db.executescript('''
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=FULL;
        PRAGMA cache_size=-32768;
        CREATE TABLE IF NOT EXISTS meta(name TEXT PRIMARY KEY,value TEXT);
        CREATE TABLE IF NOT EXISTS nodes(k BLOB PRIMARY KEY,lo INTEGER NOT NULL,hi INTEGER NOT NULL,
          info BLOB,quality INTEGER NOT NULL DEFAULT 0,chosen TEXT) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS reach(k BLOB,color INTEGER,ply INTEGER,done INTEGER DEFAULT 0,
          PRIMARY KEY(k,color)) WITHOUT ROWID;
        CREATE INDEX IF NOT EXISTS frontier ON reach(done,ply,k);
        CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,host TEXT,started REAL,elapsed REAL,
          count INTEGER DEFAULT 0,error TEXT,active INTEGER DEFAULT 1,kind TEXT);
        ''')
        if 'kind' not in {r[1] for r in self.db.execute('PRAGMA table_info(jobs)')}:
            self.db.execute('ALTER TABLE jobs ADD COLUMN kind TEXT')

    def getmeta(self, name, default=None):
        row=self.db.execute('SELECT value FROM meta WHERE name=?',(name,)).fetchone()
        return json.loads(row[0]) if row else default

    def meta(self, name, value):
        self.db.execute('INSERT OR REPLACE INTO meta VALUES(?,?)',(name,json.dumps(value)))

    def merge(self, key, lo=-64, hi=64):
        bounds({'lower':lo,'upper':hi})
        k=pack(key)
        n=self.db.execute('SELECT lo,hi FROM nodes WHERE k=?',(k,)).fetchone()
        if n:
            lo,hi=max(lo,n[0]),min(hi,n[1])
            if lo>hi: raise ValueError('contradictory proof at '+key)
            if (lo,hi)!=(n[0],n[1]): self.db.execute('UPDATE nodes SET lo=?,hi=? WHERE k=?',(lo,hi,k))
        else:
            self.db.execute('INSERT INTO nodes(k,lo,hi) VALUES(?,?,?)',(k,lo,hi))

    def add_reach(self, key, color, ply):
        self.merge(key)
        # A shortest path determines coverage; passes do not increase ply.
        self.db.execute('''INSERT INTO reach(k,color,ply) VALUES(?,?,?)
          ON CONFLICT(k,color) DO UPDATE SET ply=min(ply,excluded.ply),
          done=CASE WHEN excluded.ply<ply THEN 0 ELSE done END''',(pack(key),color,ply))

    def info(self, info):
        self.merge(info['key'])
        for c in info['children']: self.merge(c['key'])
        self.db.execute('UPDATE nodes SET info=? WHERE k=?',(encode(info),pack(info['key'])))
        if info['terminal']:
            self.merge(info['key'],info['terminalValue'],info['terminalValue'])
            self.db.execute('UPDATE nodes SET quality=2 WHERE k=?',(pack(info['key']),))

    def request(self, k):
        n=self.db.execute('SELECT * FROM nodes WHERE k=?',(k,)).fetchone()
        info=decode(n['info'])
        mode='all' if self.db.execute('SELECT 1 FROM reach WHERE k=? AND color!=? LIMIT 1',(k,info['side'])).fetchone() else 'best'
        children=[]
        for c in info['children']:
            v=self.db.execute('SELECT lo,hi FROM nodes WHERE k=?',(pack(c['key']),)).fetchone()
            children.append(dict(key=c['key'],lower=v[0],upper=v[1]))
        return dict(key=unpack(k),mode=mode,lower=n['lo'],upper=n['hi'],children=children)

    def accept(self, r, req):
        if (r.get('version')!=1 or r.get('perspective')!='side-to-move'
            or r.get('key')!=req['key'] or r.get('mode')!=req['mode']):
            raise ValueError('result identity mismatch')
        lo,hi=bounds(r)
        if req['mode'] in ['value','bound']:
            if r.get('children')!=[] or r.get('threshold')!=req.get('threshold'): raise ValueError('value job identity')
            complete=lo==hi if req['mode']=='value' else lo>=req['threshold'] or hi<req['threshold']
            if r['complete']!=complete: raise ValueError('false value completion')
            self.merge(r['key'],lo,hi)
            return complete
        info=decode(self.db.execute('SELECT info FROM nodes WHERE k=?',(pack(r['key']),)).fetchone()[0])
        expected={c['key']:c['moves'] for c in info['children']}
        if len(r['children'])!=len(expected) or {c['key']:c['moves'] for c in r['children']}!=expected:
            raise ValueError('result legal children mismatch')
        for c in r['children']: bounds(c)
        cs=r['children']
        if cs and (lo<max(-c['upper'] for c in cs) or hi>max(-c['lower'] for c in cs)):
            # A parent may independently have stronger bounds, never weaker
            # than its children once the result claims completion.
            if r['complete']: raise ValueError('unpropagated completed result')
        complete=lo==hi and all(c['lower']==c['upper'] if req['mode']=='all'
                   else c['lower']> -lo or c['lower']==c['upper']==-lo for c in cs)
        if info['terminal'] and (lo,hi)!=(info['terminalValue'],)*2:
            raise ValueError('terminal result mismatch')
        if r['complete']!=complete: raise ValueError('false completion')
        if complete and cs and not any(c['lower']==c['upper']==-lo for c in cs):
            raise ValueError('no attaining move')
        self.merge(r['key'],lo,hi)
        for c in cs: self.merge(c['key'],*bounds(c))
        if complete:
            ties=sorted(m for c in cs if c['lower']==c['upper']==-lo for m in c['moves'])
            self.db.execute('UPDATE nodes SET quality=max(quality,?),chosen=? WHERE k=?',
                            (2 if req['mode']=='all' else 1,ties[0] if ties else None,pack(r['key'])))
        return complete

    def advance(self, k, target):
        n=self.db.execute('SELECT * FROM nodes WHERE k=?',(k,)).fetchone()
        info=decode(n['info'])
        rows=self.db.execute('SELECT color,ply FROM reach WHERE k=? AND done=0',(k,)).fetchall()
        for color,ply in rows:
            need=1 if color==info['side'] else 2
            if n['quality']<need: continue
            edges=info['children'] if color!=info['side'] else [c for c in info['children'] if n['chosen'] in c['moves']]
            for c in edges:
                next_ply=ply+(c['moves']!=['pass'])
                if next_ply<=target: self.add_reach(c['key'],color,next_ply)
            self.db.execute('UPDATE reach SET done=1 WHERE k=? AND color=?',(k,color))

    def records(self, path):
        """Stream enriched parent records from a single read transaction."""
        tmp=Path(str(path)+'.tmp')
        self.db.execute('BEGIN')
        try:
            with tmp.open('w') as f:
                for n in self.db.execute('SELECT * FROM nodes WHERE info IS NOT NULL AND quality>0 ORDER BY k'):
                    info=decode(n['info']);children=[]
                    for c in info['children']:
                        b=self.db.execute('SELECT lo,hi FROM nodes WHERE k=?',(pack(c['key']),)).fetchone()
                        children.append(dict(key=c['key'],lower=b[0],upper=b[1]))
                    f.write(json.dumps(dict(treeVersion=1,key=unpack(n['k']),lower=n['lo'],upper=n['hi'],bestMove=n['chosen'],mode="all" if n['quality']==2 else "best",children=children),separators=(',',':'))+'\n')
                f.flush();os.fsync(f.fileno())
        finally: self.db.rollback()
        os.replace(tmp,path)

    def snapshot(self, path):
        # One MVCC snapshot, compact rows; memory is independent of tree size.
        tmp=Path(str(path)+'.tmp')
        with self.db, tmp.open('w') as f:
            for n in self.db.execute('SELECT k,lo,hi,chosen FROM nodes WHERE lo> -64 OR hi<64 ORDER BY k'):
                f.write(json.dumps(dict(treeVersion=1,key=unpack(n[0]),lower=n[1],upper=n[2],bestMove=n[3]),separators=(',',':'))+'\n')
            f.flush();os.fsync(f.fileno())
        os.replace(tmp,path)
        fd=os.open(Path(path).resolve().parent,os.O_RDONLY)
        try: os.fsync(fd)
        finally: os.close(fd)

class Tree:
    def __init__(self, a, store):
        self.a=a;self.s=store;self.db=store.db
        self.hosts=([dict(name=f'local-{i}',local=True,threads=1,tt_mb=16,critical=True) for i in range(a.local_hosts)]
                    if a.local_test else json.loads(Path(a.hosts).read_text())['hosts'])
        if not self.hosts or len({h['name'] for h in self.hosts})!=len(self.hosts): raise ValueError('host names')
        if any(not 1<=h['threads']<=256 or not 1<=h['tt_mb']<=32768 for h in self.hosts): raise ValueError('host capacity')
        if not any(h.get('critical') for h in self.hosts):
            for h in self.hosts: h['critical']=True
        self.queue=JobQueue(grace=a.critical_grace)
        self.serial=0;self.failed_once=False;self.completed=0
        self.active=set();self.errors=[];self.attempt_counts={}
        self.host_seconds={};self.stopping=False

    async def inspect(self, keys=None):
        args=['--requests',json.dumps(keys)] if keys is not None else (['--json',self.a.json] if self.a.json else [self.a.sequence])
        p=await asyncio.create_subprocess_exec(self.a.binary,'--type','tree-inspect',*args,stdout=asyncio.subprocess.PIPE)
        out,_=await p.communicate()
        if p.returncode: raise RuntimeError('engine inspect failed')
        return [json.loads(l) for l in out.splitlines()]

    async def seed(self):
        if self.s.getmeta('seeded'): return
        paths=[(self.a.proof,'proof')]+[(p,'book') for p in self.a.book]
        for path,kind in paths:
            if not Path(path).exists():
                if kind=='proof': raise ValueError('missing initial proof')
                continue
            print('Read-only seed:',path,flush=True)
            p=await asyncio.create_subprocess_exec(self.a.binary,'--type','tree-seed','--source',path,'--format',kind,stdout=asyncio.subprocess.PIPE)
            count=0
            try:
                async for line in p.stdout:
                    r=json.loads(line);self.s.merge(r['key'],*bounds(r));count+=1
                    if count%10000==0: self.db.commit()
                if await p.wait(): raise RuntimeError('seed conversion failed')
            finally:
                if p.returncode is None: p.kill();await p.wait()
            self.db.commit()
        self.s.meta('seeded',True);self.db.commit()

    async def cancel(self, host, attempt):
        if host.get('local'):
            directory=Path(self.a.state).resolve().parent/'dist/running'/attempt
            if directory.exists(): (directory/'cancel').touch()
        else:
            cmd=f'cd {remote_cd(host)} && python3 dist/run_host.py --cancel {shlex.quote(attempt)}'
            p=await asyncio.create_subprocess_exec(*SSH,host.get('ssh',host['name']),cmd,stdout=asyncio.subprocess.DEVNULL,stderr=asyncio.subprocess.DEVNULL)
            try: await asyncio.wait_for(p.wait(),15)
            except asyncio.TimeoutError: p.kill();await p.wait()

    async def run_batch(self, host, requests):
        attempt=uuid.uuid4().hex;start=time.time();count=0
        base=['--type','tree-batch','--selectivity','0','--threads',str(host['threads']),'--tt-mb',str(host['tt_mb']),
              '--time',str(int(self.a.timeout*1000))]
        supervisor=['python3','dist/run_host.py','--input-json','--timeout',str(self.a.timeout+5),attempt,'--']
        cwd=Path(self.a.state).resolve().parent
        if host.get('local'):
            cmd=[sys.executable,str(HERE/'run_host.py'),*supervisor[2:],self.a.binary,*base]
        else:
            gate=shlex.join(['python3','-c',"import json; assert json.load(open('dist/ready.json')).get('treeProtocol') == 1"])
            cmd=[*SSH,host.get('ssh',host['name']),f'cd {remote_cd(host)} && {gate} && {shlex.join(supervisor)} {host["command"]} {shlex.join(base)}']
        kind=requests[0]['mode'] if len({r['mode'] for r in requests})==1 else 'analysis'
        self.db.execute('INSERT INTO jobs(id,host,started,kind) VALUES(?,?,?,?)',(attempt,json.dumps(host),start,kind));self.db.commit()
        p=None;beats=None;errors=None;success=False
        try:
            p=await asyncio.create_subprocess_exec(*cmd,cwd=cwd,stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE,
                 stderr=asyncio.subprocess.PIPE,start_new_session=True,env=dict(os.environ,DOSELLO_QOS='0'),limit=1024*1024)
            p.stdin.write((json.dumps(requests,separators=(',',':'))+'\n').encode());await p.stdin.drain()
            async def heartbeat():
                while True:
                    p.stdin.write(b'.\n');await p.stdin.drain();await asyncio.sleep(5)
            async def log():
                async for line in p.stderr: print(f'[{host["name"]}] {line.decode().rstrip()}',file=sys.stderr)
            beats=asyncio.create_task(heartbeat());errors=asyncio.create_task(log())
            if self.a.simulate_failure and not self.failed_once:
                self.failed_once=True;raise RuntimeError('simulated failure')
            async def consume():
                nonlocal count
                async for line in p.stdout:
                    if count>=len(requests): raise ValueError('extra result')
                    r=json.loads(line)
                    if r.get('progress'):
                        if requests[count]['mode'] not in ['value','bound']: raise ValueError('unexpected progress record')
                        with self.db: self.s.accept(r,requests[count])
                        continue
                    with self.db:
                        complete=self.s.accept(r,requests[count])
                        self.db.execute('UPDATE jobs SET count=? WHERE id=?',(count+int(complete),attempt))
                    if not complete: raise RuntimeError('incomplete result (partial bounds committed)')
                    count+=1;self.completed+=1
                    if self.a.stop_after and self.completed>=self.a.stop_after:
                        self.stopping=True;return
            await asyncio.wait_for(consume(),self.a.timeout+15)
            if self.stopping: return
            await asyncio.wait_for(p.wait(),5)
            if p.returncode or count!=len(requests): raise RuntimeError(f'batch exit {p.returncode}, {count}/{len(requests)} results')
            success=True
            self.host_seconds[host['name']]=(time.time()-start)/max(count,1)
        finally:
            if beats:
                beats.cancel()
                with contextlib.suppress(asyncio.CancelledError,BrokenPipeError,ConnectionResetError): await beats
            if p and p.stdin: p.stdin.close()
            if not success: await self.cancel(host,attempt)
            if p and p.returncode is None:
                try: await asyncio.wait_for(p.wait(),4)
                except asyncio.TimeoutError:
                    with contextlib.suppress(ProcessLookupError): os.killpg(p.pid,signal.SIGTERM)
                    try: await asyncio.wait_for(p.wait(),3)
                    except asyncio.TimeoutError: os.killpg(p.pid,signal.SIGKILL);await p.wait()
            if errors: await errors
            self.db.execute('UPDATE jobs SET active=0,elapsed=?,count=? WHERE id=?',(time.time()-start,count,attempt));self.db.commit()

    async def worker(self, host):
        while True:
            _,_,_,requests=await self.queue.get(host)
            keys=[pack(r["key"]) for r in requests]
            try:
                await self.run_batch(host,requests)
            except (RuntimeError,OSError,asyncio.TimeoutError) as e:
                with self.db:
                    self.db.execute('UPDATE jobs SET error=? WHERE id=(SELECT id FROM jobs WHERE host=? ORDER BY started DESC LIMIT 1)',(str(e),json.dumps(host)))
                print('Requeue:',host['name'],str(e),flush=True)
                for k in keys:
                    self.attempt_counts[k]=self.attempt_counts.get(k,0)+1
                    if self.attempt_counts[k]>=self.a.retries: self.errors.append(e)
                await asyncio.sleep(1)
            except asyncio.CancelledError: raise
            except Exception as e: self.errors.append(e)
            finally: self.active.difference_update(keys)

    async def run(self):
        root=(await self.inspect())[0]
        if self.a.local_test and root['empties']>30: raise ValueError('--local-test requires <=30 empties')
        settings=dict(version=1,root=root['key'],ply=self.a.ply,passPly='placement',tie='lexicographic-move')
        old=self.s.getmeta('settings')
        if old and old!=settings:
            if dict(old,ply=settings['ply'])!=settings or settings['ply']<old['ply']:
                raise ValueError('resume settings mismatch (root/schema or decreasing ply); use a new state')
            self.db.execute('UPDATE reach SET done=0 WHERE ply>=?',(old['ply'],))
            self.s.meta('complete',False)
        for row in self.db.execute('SELECT * FROM jobs WHERE active=1').fetchall():
            await self.cancel(json.loads(row['host']),row['id'])
        if self.db.execute('SELECT 1 FROM jobs WHERE active=1').fetchone():
            await asyncio.sleep(4 if self.a.local_test else 35)
            self.db.execute("UPDATE jobs SET active=0,error='coordinator restart: lease expired' WHERE active=1")
        self.s.meta('settings',settings)
        self.s.meta('created',self.s.getmeta('created',time.time()))
        self.s.info(root)
        for color in [1,-1]: self.s.add_reach(root['key'],color,0)
        self.db.commit()
        if not self.a.no_seed: await self.seed()
        workers=[asyncio.create_task(self.worker(h)) for h in self.hosts]
        last_status=0
        try:
            while not self.stopping:
                if self.errors: raise self.errors[0]
                row=self.db.execute('SELECT min(ply) FROM reach WHERE done=0').fetchone()
                ply=row[0]
                if ply is None:
                    self.s.meta('complete',True);self.db.commit();break
                candidates=self.db.execute('''SELECT DISTINCT n.k,n.info FROM reach r JOIN nodes n USING(k)
                  WHERE r.done=0 AND r.ply=? LIMIT ?''',(ply,max(256,len(self.hosts)*self.a.batch*2))).fetchall()
                missing=[unpack(r['k']) for r in candidates if r['info'] is None]
                for off in range(0,len(missing),256):
                    for info in await self.inspect(missing[off:off+256]): self.s.info(info)
                ready=[]
                for r in candidates:
                    k=r['k']
                    if k in self.active: continue
                    self.s.advance(k,self.a.ply)
                    if not self.db.execute('SELECT 1 FROM reach WHERE k=? AND done=0',(k,)).fetchone(): continue
                    req=self.s.request(k)
                    # Fully proven seeds can certify a node without any search.
                    n=self.db.execute('SELECT info FROM nodes WHERE k=?',(k,)).fetchone()
                    info=decode(n[0]);cs=[]
                    for c,b in zip(info['children'],req['children']): cs.append(dict(c,lower=b['lower'],upper=b['upper']))
                    lo=max([req['lower']]+[-c['upper'] for c in cs]);hi=min(req['upper'],max([-c['lower'] for c in cs],default=req['upper']))
                    if lo==hi and all(c['lower']==c['upper'] if req['mode']=='all' else c['lower']> -lo or c['lower']==c['upper']==-lo for c in cs):
                        self.s.accept(dict(version=1,perspective='side-to-move',key=req['key'],mode=req['mode'],lower=lo,upper=hi,children=cs,complete=True),req)
                        self.s.advance(k,self.a.ply)
                    else:
                        self.s.merge(req['key'],lo,hi)
                        req.update(lower=lo,upper=hi)
                        if info['empties']<=self.a.analyze_empties:
                            ready.append(req)
                        elif req['mode']=='best' and lo!=hi:
                            ready.append(dict(key=req['key'],mode='value',lower=lo,upper=hi))
                        else:
                            for c in req['children']:
                                if req['mode']=='best':
                                    self.s.merge(c['key'],max(c['lower'],-lo),c['upper'])
                                    c=dict(c,lower=max(c['lower'],-lo))
                                    if c['lower']> -lo or c['lower']==c['upper']==-lo: continue
                                    task=dict(c,mode='bound',threshold=1-lo)
                                else:
                                    if c['lower']==c['upper']: continue
                                    task=dict(c,mode='value')
                                ready.append(task)
                self.db.commit()
                # Keep only a bounded window of jobs/futures resident in RAM.
                capacity=len(self.hosts)*2-len(self.active)//max(1,self.a.batch)
                batch=self.a.batch
                if self.host_seconds:
                    batch=max(1,min(batch,int(self.a.batch_seconds/(sum(self.host_seconds.values())/len(self.host_seconds)))))
                unique={}
                for req in ready:
                    if pack(req['key']) not in self.active: unique.setdefault(req['key'],req)
                ready=list(unique.values())
                # Expensive shallow value/bound requests occupy separate slots.
                if any(r['mode'] in ['value','bound'] for r in ready): batch=1
                for off in range(0,min(len(ready),max(0,capacity)*batch),batch):
                    requests=ready[off:off+batch];self.active.update(pack(r['key']) for r in requests);self.serial+=1
                    await self.queue.put((0 if ply<3 else 1,ply,self.serial,requests))
                if time.monotonic()-last_status>30:
                    print(json.dumps(dict(ply=ply,active=len(self.active),completedThisRun=self.completed)),flush=True);last_status=time.monotonic()
                await asyncio.sleep(.05 if not self.active else .2)
        finally:
            for w in workers: w.cancel()
            await asyncio.gather(*workers,return_exceptions=True)
            self.db.commit()
        print(json.dumps(dict(complete=self.s.getmeta('complete',False),state=self.a.state)),flush=True)

def parser():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--state',default=str(HERE/'tree-state.sqlite'))
    p.add_argument('--binary',default=str(BINARY))
    p.add_argument('--hosts',default=str(HERE/'hosts.json'))
    p.add_argument('--proof',default=str(RUST.parent/'proof/initial-proof.json'))
    p.add_argument('--book',action='append')
    p.add_argument('--no-seed',action='store_true')
    p.add_argument('--json');p.add_argument('--sequence',default='')
    p.add_argument('--ply',type=int,default=12)
    p.add_argument('--batch',type=int,default=64)
    p.add_argument('--analyze-empties',type=int,default=38,help='above this split per-child proofs across hosts')
    p.add_argument('--batch-seconds',type=float,default=60)
    p.add_argument('--timeout',type=float,default=86400)
    p.add_argument('--critical-grace',type=float,default=60)
    p.add_argument('--retries',type=int,default=3)
    p.add_argument('--local-test',action='store_true')
    p.add_argument('--local-hosts',type=int,choices=[1,2],default=2)
    p.add_argument('--simulate-failure',action='store_true')
    p.add_argument('--stop-after',type=int,default=0)
    p.add_argument('--dry-run',action='store_true')
    p.add_argument('--records',metavar='JSONL',help='read-only enriched snapshot for bounded-memory exporter')
    p.add_argument('--snapshot',metavar='JSONL',help='read-only compact snapshot; no solver/SSH')
    return p

def main():
    a=parser().parse_args()
    a.binary=str(Path(a.binary).resolve());a.book=a.book if a.book is not None else [str(RUST/'book/book.jsonl'),str(RUST/'book/grow.jsonl')]
    if not 0<=a.ply<=28 or not 0<=a.analyze_empties<=56 or a.critical_grace<0 or not 1<=a.batch<=256 or a.timeout<=0 or a.batch_seconds<=0 or a.retries<1: raise SystemExit('invalid scheduling limits')
    if a.dry_run:
        print(json.dumps(vars(a),indent=2));return
    if a.snapshot or a.records:
        db=sqlite3.connect(f'file:{Path(a.state).resolve()}?mode=ro',uri=True);db.row_factory=sqlite3.Row
        s=Store.__new__(Store);s.db=db
        if a.records: s.records(a.records)
        else: s.snapshot(a.snapshot)
        db.close();return
    if (a.simulate_failure or a.stop_after) and not a.local_test: raise SystemExit('test hooks require --local-test')
    if a.local_test and a.state==str(HERE/'tree-state.sqlite'): raise SystemExit('local tests require a separate --state')
    path=Path(a.state).resolve();path.parent.mkdir(parents=True,exist_ok=True)
    with open(str(path)+'.lock','a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB);lock.seek(0);lock.truncate();lock.write(str(os.getpid()));lock.flush()
        s=Store(path)
        async def run():
            loop=asyncio.get_running_loop();task=asyncio.current_task()
            for sig in [signal.SIGTERM,signal.SIGINT]: loop.add_signal_handler(sig,task.cancel)
            await Tree(a,s).run()
        try: asyncio.run(run())
        except asyncio.CancelledError: print('Stopped; committed results retained.')
        finally: s.db.close()

if __name__=='__main__': main()
