#!/usr/bin/env python3
"""Check the locally served Pages tree with curl. Loopback connections only."""
from pathlib import Path
import argparse
import subprocess
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--port', type=int, default=8000)
a = p.parse_args()
if not 1 <= a.port <= 65535:
    p.error('invalid port')
docs = Path(__file__).resolve().parent.parent/'docs'
paths = ['/', '/play/'] + ['/'+f.relative_to(docs).as_posix() for f in sorted(docs.rglob('*')) if f.is_file()]
for path in paths:
    r = subprocess.run(['curl', '--noproxy', '*', '--max-time', '5', '--fail', '--silent', '--show-error',
                        '--output', '/dev/null', '--write-out', '%{http_code}', f'http://127.0.0.1:{a.port}{path}'],
                       capture_output=True, text=True)
    if r.returncode or r.stdout != '200':
        raise SystemExit(f'FAIL {path}: HTTP {r.stdout}: {r.stderr}')
print(f'PASS HTTP 200: {len(paths)-2} assets and both page routes')
