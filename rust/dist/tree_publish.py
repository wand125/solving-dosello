#!/usr/bin/env python3
"""Publish exact tree progress in 20,000,000-byte steps, without running solvers.
An existing grow-steps controller and this publisher cannot own exports/book together.
The default destination is separate for review. No builder processes are touched.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import time
from tree import Store, RUST
from coordinator import atomic

ROOT=RUST.parent
STEP=20_000_000

def run(*args):
    subprocess.run(list(map(str,args)),check=True,cwd=ROOT)

def publish(a, step):
    out=Path(a.output).resolve()
    current=json.loads((out/'index.json').read_text()) if (out/'index.json').exists() else None
    base=(out/current['build']) if current else (Path(a.base).resolve() if a.base else None)
    staging=out/f'.tree-staging-{os.getpid()}'
    snapshot=out/f'.tree-records-{os.getpid()}.jsonl'
    if staging.exists(): shutil.rmtree(staging) # only this PID's private staging
    db=sqlite3.connect(f'file:{Path(a.state).resolve()}?mode=ro',uri=True);db.row_factory=sqlite3.Row
    store=Store.__new__(Store);store.db=db
    try: store.records(snapshot)
    finally: db.close()
    try:
        command=[a.exporter,'--tree-records',snapshot,'--output',staging,'--max-bytes',str(step)]
        if base: command+=['--base-shards',base]
        run(*command)
        info=json.loads((staging/'index.json').read_text())
        if info['bytes']<step-1024 and not a.final:
            return False,info
        if not info['treeSamples']:
            raise RuntimeError('no tree record selected; publication would not include tree coverage')
        run('node',RUST/'tools/validate-shards.mjs',staging,'--all-shards')
        (staging/'.tree-owned').write_text('tree_publish v1\n')
        # All written shards are fsynced before the publication commit point.
        for directory,_,files in os.walk(staging):
            for name in files:
                with open(Path(directory)/name,'rb') as f: os.fsync(f.fileno())
            fd=os.open(directory,os.O_RDONLY)
            try: os.fsync(fd)
            finally: os.close(fd)
        version=out/info['build']
        if version.exists(): shutil.rmtree(staging)
        else: staging.rename(version)
        previous=current['build'] if current else None
        atomic(out/'index.json',info)
        with (out/'tree-publications.jsonl').open('a') as f:
            f.write(json.dumps(dict(at=time.time(),previous=previous,**info))+'\n');f.flush();os.fsync(f.fileno())
        # Only collect generations created by this publisher. Existing stage-1
        # versions remain owned by their original controller.
        for old in out.glob('v*'):
            if old.name not in [previous,info['build']] and (old/'.tree-owned').is_file():
                shutil.rmtree(old)
        return True,info
    finally:
        snapshot.unlink(missing_ok=True)
        if staging.exists(): shutil.rmtree(staging)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--state',default=str(RUST/'dist/tree-state.sqlite'))
    p.add_argument('--output',default=str(ROOT/'exports/book-tree'))
    p.add_argument('--base',help='immutable existing shard build directory (initial/PV records required)')
    p.add_argument('--exporter',default=str(RUST/'target/release/book_export_shards'))
    p.add_argument('--once',action='store_true')
    p.add_argument('--final',action='store_true',help='explicitly publish final partial step')
    p.add_argument('--poll',type=float,default=120)
    p.add_argument('--until-percent',type=int,default=100)
    p.add_argument('--test-step-bytes',type=int,help='test destinations only; never exports/book')
    a=p.parse_args()
    a.exporter=str(Path(a.exporter).resolve());out=Path(a.output).resolve()
    step=a.test_step_bytes or STEP
    if step<1024 or not 1<=a.until_percent<=100 or a.poll<=0: p.error('invalid publication limits')
    live=out==ROOT/'exports/book'
    if live and a.test_step_bytes: p.error('test step forbidden for live exports/book')
    out.mkdir(parents=True,exist_ok=True)
    shared=RUST/'results/grow-steps.lock'
    with (out/'.tree-publish.lock').open('a+') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        lock.seek(0);lock.truncate();lock.write(str(os.getpid()));lock.flush()
        if live:
            try: shared.mkdir()
            except FileExistsError: raise SystemExit('grow-steps owns exports/book; choose --output exports/book-tree or stop that controller yourself before launching this publisher')
        try:
            while True:
                current=json.loads((out/'index.json').read_text()) if (out/'index.json').exists() else (json.loads((Path(a.base)/'index.json').read_text()) if a.base else {'bytes':0})
                target=(round(current['bytes']/step)+1)*step
                if target>step*a.until_percent: break
                ok,info=publish(a,target)
                print(json.dumps(dict(published=ok,target=target,available=info['bytes'],build=info['build'])),flush=True)
                if a.once or a.final: break
                if not ok: time.sleep(a.poll)
        finally:
            if live: shared.rmdir()

if __name__=='__main__': main()
