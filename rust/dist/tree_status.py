#!/usr/bin/env python3
"""Read-only solution-tree coverage and measured throughput. No SSH."""
import argparse
import json
from pathlib import Path
import sqlite3
import time


def report(path):
    db=sqlite3.connect(f'file:{Path(path).resolve()}?mode=ro',uri=True)
    known={(c,d):(n,s) for c,d,n,s in db.execute('SELECT color,ply,count(*),sum(done) FROM reach GROUP BY color,ply')}
    setting=db.execute("SELECT value FROM meta WHERE name='settings'").fetchone()
    target=json.loads(setting[0])['ply'] if setting else 0
    rows=[(c,d,*known.get((c,d),(0,0))) for d in range(target+1) for c in [1,-1]]
    complete=db.execute("SELECT value FROM meta WHERE name='complete'").fetchone()
    complete=bool(complete and json.loads(complete[0]))
    print('AI     ply       needed       solved      pending')
    for color,ply,total,done in rows:
        print(f'{"black" if color==1 else "white":5} {ply:4} {total:12,} {done:12,} {total-done:12,}')
    print('needed = discovered reachable nodes, not an extrapolated final tree size')
    completed=db.execute('SELECT count(*),coalesce(sum(count),0),coalesce(sum(elapsed),0) FROM jobs WHERE active=0 AND error IS NULL').fetchone()
    failures=db.execute('SELECT count(*) FROM jobs WHERE error IS NOT NULL').fetchone()[0]
    pending=db.execute('SELECT count(DISTINCT k) FROM reach WHERE done=0').fetchone()[0]
    print(f'jobs={completed[0]} completed work items={completed[1]} failures={failures} unique pending={pending}')
    rates=[]
    for host,n,elapsed in db.execute('SELECT host,sum(count),sum(elapsed) FROM jobs WHERE active=0 AND error IS NULL GROUP BY host'):
        h=json.loads(host);rate=n/elapsed if elapsed else 0;
        print(f'{h["name"]}: measured {rate:.4f} work-items/s ({n} items, {elapsed:.1f}s including transport/TT)')
    columns={r[1] for r in db.execute('PRAGMA table_info(jobs)')}
    if 'kind' in columns:
        for host,n,elapsed in db.execute("SELECT host,sum(count),sum(elapsed) FROM jobs WHERE active=0 AND error IS NULL AND kind IN ('all','best','analysis') GROUP BY host"):
            rates.append(n/elapsed if elapsed else 0)
    if complete:
        print("ETA: 0s (tree complete)")
    elif sum(rates):
        print(f'Frontier analysis ETA proxy: {pending/sum(rates):.0f}s; based on parent-analysis jobs only; excludes undiscovered descendants and shallow child proofs.')
    else: print('ETA unavailable: no completed parent-analysis measurements')
    for host,started,count in db.execute('SELECT host,started,count FROM jobs WHERE active=1'):
        print(f'{json.loads(host)["name"]}: active {time.time()-started:.0f}s, {count} work items committed')
    print('complete='+str(complete))
    db.close()

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--state',default=str(Path(__file__).with_name('tree-state.sqlite')))
    report(p.parse_args().state)
