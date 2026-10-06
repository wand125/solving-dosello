#!/usr/bin/env python3
"""Aggregate published job results; never search or contact workers."""
import collections
import datetime
import json
from pathlib import Path
root = Path(__file__).resolve().parent.parent
p = json.loads((root/'proof/initial-proof.json').read_text())
jobs = p['jobs']
jst = datetime.timezone(datetime.timedelta(hours=9))
print(json.dumps({
    'proofNodes': len(p['nodes']),
    'jobs': len(jobs),
    'distinctJobPositions': len({j['id'] for j in jobs}),
    'incompleteJobs': sum(not j['result']['complete'] for j in jobs),
    'recordedFailures': len(p.get('failures', [])),
    'nodesSearched': sum(j['result']['nodes'] for j in jobs),
    'threadHours': sum(j['elapsed'] * j['host']['threads'] for j in jobs) / 3600,
    'leafEmpties': dict(collections.Counter(j['empties'] for j in jobs)),
    'createdJST': datetime.datetime.fromtimestamp(p['created'], jst).isoformat(),
    'updatedJST': datetime.datetime.fromtimestamp(p['updated'], jst).isoformat(),
    'hardwareSource': 'Task-supplied inventory; CPU models are not encoded in the proof.'
}, indent=2))
