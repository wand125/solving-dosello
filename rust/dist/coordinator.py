#!/usr/bin/env python3
"""Resumable best-first negamax proof; this process never performs a search.
`job --type inspect` supplies legal children, symmetries and static estimates only.
Completed full-depth bounds survive retries, cancellation and coordinator restart.
"""
import argparse
import asyncio
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shlex
import signal
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
RUST = HERE.parent
DEFAULT_BINARY = RUST / 'target/release/job'
SSH = ['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', '-o', 'ServerAliveInterval=10', '-o', 'ServerAliveCountMax=2']


def atomic(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + '.tmp')
    with tmp.open('w') as f:
        json.dump(data, f, separators=(',', ':'), sort_keys=True)
        f.write('\n')
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def identity(position):
    return hashlib.sha256(json.dumps(position, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def remote_cd(host):
    directory = host['directory']
    if directory.startswith('~/'):
        return '"$HOME"/' + shlex.quote(directory[2:])
    return shlex.quote(directory)


class JobQueue:
    """Priority queue where critical (prio 0) jobs go to hosts marked critical
    (the fastest slots). Other hosts take a critical job only after it has
    waited `grace` seconds, so a busy fast host never stalls the proof."""
    def __init__(self, grace=600):
        self.items = []
        self.cond = asyncio.Condition()
        self.grace = grace

    async def put(self, item):
        async with self.cond:
            self.items.append((item, time.monotonic()))
            self.cond.notify_all()

    async def get(self, host):
        async with self.cond:
            while True:
                now = time.monotonic()
                ok = [(item, t) for item, t in self.items
                      if item[0] != 0 or host.get('critical') or now - t >= self.grace]
                if ok:
                    choice = min(ok, key=lambda x: x[0][:3])
                    self.items.remove(choice)
                    return choice[0]
                with contextlib.suppress(asyncio.TimeoutError):
                    await asyncio.wait_for(self.cond.wait(), 30)

    def task_done(self):
        pass


class Coordinator:
    def __init__(self, args):
        self.a = args
        self.path = Path(args.state).resolve()
        self.binary = str(Path(args.binary).resolve())
        self.hosts = ([{'name': f'local-{i}', 'threads': 1, 'tt_mb': 16, 'local': True} for i in range(args.local_hosts)]
                      if args.local_test else json.loads(Path(args.hosts).read_text())['hosts'])
        if not self.hosts or len({h['name'] for h in self.hosts}) != len(self.hosts):
            raise ValueError('empty/duplicate hosts')
        if not any(h.get('critical') for h in self.hosts):
            for h in self.hosts:  # no preference configured: every slot may take critical jobs
                h['critical'] = True
        for h in self.hosts:
            if not 1 <= h['threads'] <= 256 or not 1 <= h['tt_mb'] <= 32768:
                raise ValueError('invalid host capacity')
        self.state = json.loads(self.path.read_text()) if self.path.exists() else {
            'version': 1, 'nodes': {}, 'jobs': [], 'running': {}, 'created': time.time(), 'complete': False}
        if self.state['version'] != 1:
            raise ValueError('state version mismatch')
        self.queue = JobQueue()
        self.serial = 0
        self.result_count = 0
        self.workers = []
        self.failed_once = False
        self.node_locks = {}
        self.info_locks = {}
        self.attempts = {}

    def save(self):
        self.state['updated'] = time.time()
        atomic(self.path, self.state)

    async def inspect(self, position):
        proc = await asyncio.create_subprocess_exec(self.binary, '--type', 'inspect', json.dumps(position),
                                                   stdout=asyncio.subprocess.PIPE)
        out, _ = await proc.communicate()
        if proc.returncode:
            raise RuntimeError('inspect failed')
        return json.loads(out)

    async def add(self, position):
        key = identity(position)
        async with self.info_locks.setdefault(key, asyncio.Lock()):
            return await self._add(position, key)

    async def _add(self, position, key):
        if key not in self.state['nodes']:
            info = await self.inspect(position)
            terminal = info['terminal']
            self.state['nodes'][key] = {
                'position': position, 'lower': info['terminalValue'] if terminal else -64,
                'upper': info['terminalValue'] if terminal else 64, 'info': info,
                'children': [], 'split': False}
        return key

    async def expand(self, key):
        n = self.state['nodes'][key]
        if not n['split']:
            children = []
            for edge in n['info']['children']:
                child = await self.add(edge['position'])
                children.append({'id': child, 'moves': edge['moves'], 'estimate': edge['estimate']})
            n['children'] = children
            n['split'] = True
            self.save()
        return n['children']

    def merge(self, key, lo, hi):
        n = self.state['nodes'][key]
        if not -64 <= lo <= hi <= 64:
            raise ValueError('invalid result bounds')
        lo, hi = max(n['lower'], lo), min(n['upper'], hi)
        if lo > hi:
            raise ValueError('contradictory proof')
        n['lower'], n['upper'] = lo, hi

    def propagate(self, key):
        n = self.state['nodes'][key]
        if n['children']:
            children = [self.state['nodes'][c['id']] for c in n['children']]
            self.merge(key, max(-c['upper'] for c in children), max(-c['lower'] for c in children))
            self.save()

    def propagate_all(self):
        while True:
            changed = False
            for key, n in reversed(list(self.state['nodes'].items())):
                if n['children']:
                    before = (n['lower'], n['upper'])
                    cs = [self.state['nodes'][c['id']] for c in n['children']]
                    self.merge(key, max(-c['upper'] for c in cs), max(-c['lower'] for c in cs))
                    changed |= before != (n['lower'], n['upper'])
            if not changed:
                break

    def split(self, key, depth):
        n = self.state['nodes'][key]
        if n['split']:
            return True
        if depth >= self.a.split_depth:
            return False
        timings = [j for j in self.state['jobs'] if j.get('result', {}).get('complete') and j['elapsed'] > .01]
        expected = 0
        if timings:
            # Crude cost extrapolation, used only for scheduling, never as a proof.
            j = timings[-1]
            expected = j['elapsed'] * 2 ** ((n['info']['empties']-j['empties'])/2)
        return n['info']['empties'] > self.a.split_empties or expected > self.a.split_seconds

    async def leaf(self, key, kind, threshold=0, prio=1):
        future = asyncio.get_running_loop().create_future()
        self.serial += 1
        n = self.state['nodes'][key]
        self.state.setdefault('pending', {})[f'{key}:{kind}:{threshold}'] = {
            'id': key, 'type': kind, 'threshold': threshold, 'queued': time.time()}
        self.save()
        await self.queue.put((prio, n['info']['empties'], self.serial, (key, kind, threshold, future)))
        await future
        self.result_count += 1
        if self.a.stop_after and self.result_count >= self.a.stop_after:
            raise InterruptedError('requested stop after completed results')

    async def bound(self, key, threshold, depth, prio=1):
        lock = self.node_locks.setdefault(key, asyncio.Lock())
        async with lock:
            return await self._bound(key, threshold, depth, prio)

    async def _bound(self, key, threshold, depth, prio=1):
        n = self.state['nodes'][key]
        if n['lower'] >= threshold:
            return True
        if n['upper'] < threshold:
            return False
        if not self.split(key, depth):
            await self.leaf(key, 'bound', threshold, prio)
        else:
            # OR: one refuting reply suffices. AND: all replies must fail to refute.
            for child in await self.expand(key):
                await self.bound(child['id'], 1-threshold, depth+1, prio)
                self.propagate(key)
                if n['lower'] >= threshold or n['upper'] < threshold:
                    break
        if n['lower'] >= threshold:
            return True
        if n['upper'] < threshold:
            return False
        raise RuntimeError('incomplete bound proof')

    async def exact(self, key, depth):
        async with self.node_locks.setdefault(key, asyncio.Lock()):
            n = self.state['nodes'][key]
            if n['lower'] == n['upper']:
                return n['lower']
            if not self.split(key, depth):
                await self.leaf(key, 'exact', prio=0)
            else:
                children = await self.expand(key)
                nodes = self.state['nodes']
                # Children already proven at least as good as every sibling's lower
                # bound are solved exactly in parallel with the first child.
                best_lo = max(-nodes[c['id']]['upper'] for c in children)
                first = [children[0]] + [c for c in children[1:]
                                         if best_lo > -64 and -nodes[c['id']]['upper'] >= best_lo]
                rest = [c for c in children if c not in first]
                # Speculation: while those are solved exactly, idle slots already
                # test the other siblings against the estimate. Proven bounds are
                # kept even if the guess is wrong; leftovers are cancelled.
                guess = max(children[0]['estimate'], best_lo)
                spec = [asyncio.create_task(self.bound(c['id'], -guess, depth+1, 2)) for c in rest]
                try:
                    v = max(-x for x in await asyncio.gather(*(self.exact(c['id'], depth+1) for c in first)))
                    self.propagate(key)
                    if v != guess:
                        # Wrong guess: stop queued speculation so it can't hold the
                        # child locks behind low-priority jobs (bounds proven so far stay).
                        for t in spec:
                            t.cancel()
                        await asyncio.gather(*spec, return_exceptions=True)
                    # Parallel upper-bound proofs use the incumbent at dispatch time.
                    answers = await asyncio.gather(*(self.bound(c['id'], -v, depth+1) for c in rest))
                    better = [c for c, refuted in zip(rest, answers)
                              if not refuted and nodes[c['id']]['lower'] < -v]
                    if better:
                        v = max(v, max(-x for x in await asyncio.gather(*(self.exact(c['id'], depth+1) for c in better))))
                    self.propagate(key)
                finally:
                    for t in spec:
                        t.cancel()
                    await asyncio.gather(*spec, return_exceptions=True)
            if n['lower'] != n['upper']:
                raise RuntimeError('incomplete exact proof')
            return n['lower']

    async def cancel_remote(self, host, attempt):
        if host.get('local'):
            directory = RUST/'dist/running'/attempt
            if directory.exists():
                (directory/'cancel').touch()
            return
        cmd = f'cd {remote_cd(host)} && python3 dist/run_host.py --cancel {shlex.quote(attempt)}'
        p = await asyncio.create_subprocess_exec(*SSH, host.get('ssh', host['name']), cmd,
                                                stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.DEVNULL)
        try:
            await asyncio.wait_for(p.wait(), 15)
        except asyncio.TimeoutError:
            p.kill()
            await p.wait()
            # Lost connectivity: the supervisor's finite stdin lease still expires.

    async def run_job(self, host, key, kind, threshold):
        n = self.state['nodes'][key]
        attempt = uuid.uuid4().hex
        checkpoint = f'dist/checkpoints/{key}.json'
        base = ['--type', kind, '--threshold', str(threshold), '--threads', str(host['threads']),
                '--tt-mb', str(host['tt_mb']), '--time', str(int(self.a.timeout*1000)),
                '--checkpoint', checkpoint, json.dumps(n['position'], separators=(',', ':'))]
        if host.get('local'):
            cp = self.path.parent / 'checkpoints' / (key+'.json')
            cp.parent.mkdir(exist_ok=True)
            base[base.index('--checkpoint')+1] = str(cp)
            cmd = [sys.executable, str(HERE/'run_host.py'), '--timeout', str(self.a.timeout+5), attempt, '--',
                   'nice', '-n', '10', self.binary, *base]
        else:
            command = shlex.join(['python3', 'dist/run_host.py', '--timeout', str(self.a.timeout+5), attempt, '--'])
            remote = f'cd {remote_cd(host)} && test -f dist/ready.json && mkdir -p dist/checkpoints && {command} {host["command"]} {shlex.join(base)}'
            cmd = [*SSH, host.get('ssh', host['name']), remote]  # several slots may share one ssh host
        record = {'id': key, 'type': kind, 'threshold': threshold, 'attempt': attempt,
                  'host': host, 'started': time.time(), 'empties': n['info']['empties']}
        self.state['running'][host['name']] = record
        self.save()  # Persist attempt before launch; restart can always request its cancellation.
        p = await asyncio.create_subprocess_exec(*cmd, stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
                start_new_session=True, cwd=RUST, env=dict(os.environ, DOSELLO_QOS='0'))

        async def heartbeat():
            while True:
                p.stdin.write(b'.\n')
                await p.stdin.drain()
                await asyncio.sleep(5)

        async def stderr():
            while line := await p.stderr.readline():
                print(f'[{host["name"]} {key[:8]}] {line.decode().rstrip()}', file=sys.stderr)

        beats = asyncio.create_task(heartbeat())
        errors = asyncio.create_task(stderr())
        succeeded = False
        try:
            if self.a.simulate_failure and not self.failed_once:
                self.failed_once = True
                raise RuntimeError('simulated host failure')
            out = await asyncio.wait_for(p.stdout.read(), self.a.timeout+15)
            await asyncio.wait_for(p.wait(), 5)
            if p.returncode:
                raise RuntimeError(f'job exit {p.returncode}')
            r = json.loads(out)
            if (r['version'] != 1 or r['position'] != n['position'] or r['type'] != kind
                    or r['threshold'] != threshold or r['perspective'] != 'side-to-move'):
                raise ValueError('result identity mismatch')
            lo, hi = r['lower'], r['upper']
            if type(lo) is not int or type(hi) is not int:
                raise ValueError('noninteger bounds')
            self.merge(key, lo, hi)
            record.update(result=r, elapsed=time.time()-record['started'])
            self.state['jobs'].append(record)
            self.propagate_all()
            self.save()
            if not r['complete']:
                raise RuntimeError('job timed out; saved completed bounds')
            if kind == 'exact' and lo != hi or kind == 'bound' and not (lo >= threshold or hi < threshold):
                raise ValueError('false completion')
            succeeded = True
        finally:
            beats.cancel()
            with contextlib.suppress(asyncio.CancelledError, BrokenPipeError, ConnectionResetError):
                await beats
            if p.stdin:
                p.stdin.close()  # EOF cancels remote supervisor even when the SSH session survives.
            if not succeeded:
                await self.cancel_remote(host, attempt)
            if p.returncode is None and not succeeded:
                # Give the supervisor time to terminate/reap its separate job group.
                with contextlib.suppress(asyncio.TimeoutError):
                    await asyncio.wait_for(p.wait(), 4)
            if p.returncode is None:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(p.pid, signal.SIGTERM)
                try:
                    await asyncio.wait_for(p.wait(), 3)
                except asyncio.TimeoutError:
                    os.killpg(p.pid, signal.SIGKILL)
                    await p.wait()
            await errors
            self.state['running'].pop(host['name'], None)
            self.save()

    async def worker(self, host):
        while True:
            prio, _, _, (key, kind, threshold, future) = await self.queue.get(host)
            if future.done():  # cancelled speculation: drop without running
                self.state.setdefault('pending', {}).pop(f'{key}:{kind}:{threshold}', None)
                self.queue.task_done()
                continue
            try:
                try:
                    await self.run_job(host, key, kind, threshold)
                    self.state.setdefault('pending', {}).pop(f'{key}:{kind}:{threshold}', None)
                    self.save()
                    if not future.done():
                        future.set_result(None)
                except (RuntimeError, OSError, asyncio.TimeoutError) as e:
                    attempts = self.attempts.get(id(future), 0)+1
                    self.attempts[id(future)] = attempts
                    self.state.setdefault('failures', []).append({'host': host['name'], 'id': key, 'error': str(e), 'time': time.time()})
                    self.save()
                    print(f'requeue {host["name"]}: {e}', file=sys.stderr)
                    if attempts >= self.a.retries:
                        if not future.done():
                            future.set_exception(e)
                    else:
                        self.serial += 1
                        await self.queue.put((prio, self.state['nodes'][key]['info']['empties'], self.serial,
                                              (key, kind, threshold, future)))
                        # Other hosts can take the requeued job while this host cools down.
                        await asyncio.sleep(min(attempts, 10))
            except Exception as e:
                if not future.done():
                    future.set_exception(e)
            finally:
                self.queue.task_done()

    async def run(self):
        if self.state['running']:
            for record in list(self.state['running'].values()):
                await self.cancel_remote(record['host'], record['attempt'])
            # Wait out the remote lease if it was unreachable; no duplicate old job.
            await asyncio.sleep(4 if self.a.local_test else 35)
            self.state['running'] = {}
            self.save()
        # Durable pending entries are regenerated from the saved proof intervals.
        self.state['pending'] = {}
        if 'root' not in self.state:
            position = json.loads(Path(self.a.json).read_text()) if self.a.json else None
            if position is None:
                proc = await asyncio.create_subprocess_exec(self.binary, '--type', 'inspect', self.a.sequence,
                                                           stdout=asyncio.subprocess.PIPE)
                out, _ = await proc.communicate()
                if proc.returncode:
                    raise ValueError('invalid sequence')
                position = json.loads(out)['position']
            root = await self.add(position)
            self.state['root'] = root
        root = self.state['root']
        self.state['settings'] = {'split_empties': self.a.split_empties, 'split_depth': self.a.split_depth,
                                  'split_seconds': self.a.split_seconds, 'hosts': self.hosts}
        if self.a.sequence:
            proc = await asyncio.create_subprocess_exec(self.binary, '--type', 'inspect', self.a.sequence, stdout=asyncio.subprocess.PIPE)
            out, _ = await proc.communicate()
            if proc.returncode or identity(json.loads(out)['position']) != root:
                raise ValueError('resume sequence mismatch')
        if self.a.json and identity(json.loads(Path(self.a.json).read_text())) != root:
            raise ValueError('resume position mismatch')
        if self.a.local_test and self.state['nodes'][root]['info']['empties'] > 34:
            raise ValueError('--local-test requires <=34 empties')
        await self.expand(root)  # Root always decomposed; M4 only inspects, never solves.
        self.save()
        self.workers = [asyncio.create_task(self.worker(h)) for h in self.hosts]
        try:
            value = await self.exact(root, 0)
            # Enumerate ALL ties: <= incumbent alone would only prove one attaining move.
            children = self.state['nodes'][root]['children']
            worse = await asyncio.gather(*(self.bound(c['id'], 1-value, 1) for c in children))
            best = [m for c, no in zip(children, worse) if not no for m in c['moves']]
            self.state.update(complete=True, value=value, bestMoves=best, perspective='side-to-move')
            self.propagate(root)
            self.save()
            atomic(self.a.proof, self.state)
            print(json.dumps({'complete': True, 'value': value, 'bestMoves': best, 'proof': self.a.proof}))
        finally:
            for worker in self.workers:
                worker.cancel()
            await asyncio.gather(*self.workers, return_exceptions=True)


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', default=str(DEFAULT_BINARY))
    p.add_argument('--hosts', default=str(HERE/'hosts.json'))
    p.add_argument('--state', default=str(HERE/'state.json'))
    p.add_argument('--proof', default=str(HERE/'computed-proof.json'))
    p.add_argument('--sequence', default='')
    p.add_argument('--json')
    p.add_argument('--split-empties', type=int, default=50)
    p.add_argument('--split-depth', type=int, default=3, help='maximum plies from root (root is always split)')
    p.add_argument('--split-seconds', type=float, default=3600)
    p.add_argument('--timeout', type=float, default=86400)
    p.add_argument('--retries', type=int, default=3)
    p.add_argument('--local-test', action='store_true')
    p.add_argument('--local-hosts', type=int, choices=[1, 2], default=2)
    p.add_argument('--dry-run', action='store_true')
    p.add_argument('--simulate-failure', action='store_true')
    p.add_argument('--stop-after', type=int, default=0)
    return p


def main():
    a = parser().parse_args()
    if a.timeout <= 0 or a.retries < 1 or a.split_depth < 1 or a.split_empties < 0:
        raise SystemExit('invalid scheduling limits')
    if a.dry_run:
        print(json.dumps({'action': 'plan only; no processes started', 'hosts': json.loads(Path(a.hosts).read_text()),
                          'splitEmpties': a.split_empties, 'splitDepth': a.split_depth, 'state': a.state,
                          'proof': a.proof}, indent=2))
        return
    if (a.simulate_failure or a.stop_after) and not a.local_test:
        raise SystemExit('failure injection is local-test only')
    if a.local_test:
        if a.state == str(HERE/'state.json'):
            a.state = str(HERE/'local-state.json')
        if a.proof == str(RUST.parent/'proof/initial-proof.json'):
            a.proof = str(HERE/'local-proof.json')
    path = Path(a.state).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_suffix('.lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit('coordinator already running for this state')
        lock.write(str(os.getpid()))
        lock.flush()
        async def run():
            task = asyncio.current_task()
            loop = asyncio.get_running_loop()
            for sig in (signal.SIGTERM, signal.SIGINT):
                loop.add_signal_handler(sig, task.cancel)
            await Coordinator(a).run()
        try:
            asyncio.run(run())
        except (asyncio.CancelledError, InterruptedError):
            print('Stopped; completed bounds saved. Rerun the same command to resume.', file=sys.stderr)
        except Exception as e:
            raise SystemExit(str(e))


if __name__ == '__main__':
    main()
