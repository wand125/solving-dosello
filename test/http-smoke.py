#!/usr/bin/env python3
"""Check a locally served docs tree. Run the server bound to IPv6 loopback."""
from pathlib import Path
from http.client import HTTPConnection
import argparse
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--port', type=int, default=8000)
a = p.parse_args()
if not 1 <= a.port <= 65535:
    p.error('invalid port')
docs = Path(__file__).resolve().parent.parent/'docs'
paths = ['/', '/play/'] + ['/'+f.relative_to(docs).as_posix() for f in sorted(docs.rglob('*')) if f.is_file()]
for path in paths:
    connection = HTTPConnection('::1', a.port, timeout=5)
    connection.request('GET', path)
    response = connection.getresponse()
    if response.status != 200:
        raise SystemExit(f'FAIL {path}: HTTP {response.status}')
    response.read()
    connection.close()
print(f'PASS HTTP 200: {len(paths)-2} assets and both page routes')
