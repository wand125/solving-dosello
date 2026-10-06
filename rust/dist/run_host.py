#!/usr/bin/env python3
"""Remote job supervisor: stdin heartbeats are a renewable 30 second lease.
No daemon: EOF, lost lease, timeout, cancellation, or signal kills the job group.
The cancellation marker is scoped to a unique attempt (no recycled-PID kills).
"""
import argparse
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
import time


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cancel', action='store_true')
    p.add_argument('--input-json', action='store_true', help='read first stdin line as private job request file')
    p.add_argument('--timeout', type=float, default=86400)
    p.add_argument('attempt')
    p.add_argument('command', nargs=argparse.REMAINDER)
    a = p.parse_args()
    if not re.fullmatch(r'[a-f0-9]{32}', a.attempt):
        p.error('invalid attempt id')
    directory = Path('dist/running') / a.attempt
    if a.cancel:
        # Supervisor owns the PID and kills its live process group on this request.
        if directory.exists():
            (directory / 'cancel').touch()
        return 0
    directory.mkdir(parents=True, exist_ok=False)
    proc = None
    stopped = False

    def stop(*_):
        nonlocal stopped
        stopped = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGHUP, stop)  # SSH session teardown must also reap the job.
    try:
        command = a.command[1:] if a.command[:1] == ['--'] else a.command
        if a.input_json:
            # Unbuffered payload line, followed by arbitrary heartbeat bytes.
            # A heartbeat received in the same read already renews this lease.
            sel = selectors.DefaultSelector()
            sel.register(sys.stdin, selectors.EVENT_READ)
            payload = bytearray()
            deadline = time.monotonic() + 30
            while not payload.endswith(b'\n'):
                if stopped or time.monotonic() > deadline:
                    raise RuntimeError('request lease expired')
                if not sel.select(.2):
                    continue
                chunk = os.read(sys.stdin.fileno(), 65536)
                if not chunk:
                    raise RuntimeError('EOF before request')
                newline = chunk.find(b'\n')
                payload.extend(chunk if newline < 0 else chunk[:newline+1])
                if len(payload) > 8_000_000:
                    raise ValueError('request too large')
            sel.close()
            json.loads(payload)
            request = directory / 'input.json'
            request.write_bytes(payload)
            command += ['--request-file', str(request)]
        env = dict(os.environ, DOSELLO_QOS='0')
        proc = subprocess.Popen(command, stdin=subprocess.DEVNULL, start_new_session=True, env=env)
        (directory / 'pid.json').write_text(json.dumps({'pid': proc.pid, 'supervisor': os.getpid()}))
        sel = selectors.DefaultSelector()
        sel.register(sys.stdin, selectors.EVENT_READ)
        start = last = time.monotonic()
        while proc.poll() is None:
            now = time.monotonic()
            if stopped or (directory / 'cancel').exists() or now-last > 30 or now-start > a.timeout:
                break
            if sel.select(.2):
                if not os.read(sys.stdin.fileno(), 4096):
                    break
                last = time.monotonic()
        if proc.poll() is None:
            os.killpg(proc.pid, signal.SIGTERM)
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
        return proc.wait()
    finally:
        if proc is not None and proc.poll() is None:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
        for f in directory.iterdir():
            f.unlink()
        directory.rmdir()


if __name__ == '__main__':
    sys.exit(main())
