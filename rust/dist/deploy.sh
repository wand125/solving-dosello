#!/bin/bash
# Explicit user invocation only. This script is never run by local validation.
set -euo pipefail
if [[ $# != 1 || ! "$1" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]]; then
  echo "usage: $0 <host>" >&2
  exit 2
fi
host=$1
root=$(cd "$(dirname "$0")/.." && pwd)
ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" 'mkdir -p ~/dosello-solve/data ~/dosello-solve/dist; rm -f ~/dosello-solve/dist/ready.json'
# No --delete: existing checkpoints and native target survive deployment.
rsync -a -e 'ssh -o BatchMode=yes -o ConnectTimeout=10' "$root/src" "$root/Cargo.toml" "$root/Cargo.lock" "$host:dosello-solve/"
rsync -a -e 'ssh -o BatchMode=yes -o ConnectTimeout=10' "$root/data/eval.bin" "$root/data/probcut.csv" "$host:dosello-solve/data/"
rsync -a -e 'ssh -o BatchMode=yes -o ConnectTimeout=10' "$root/dist/run_host.py" "$root/dist/selftest.py" "$root/dist/fixtures.json" "$host:dosello-solve/dist/"
ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" 'set -eu
cd ~/dosello-solve
export DOSELLO_QOS=0 CARGO_BUILD_JOBS=2 PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
nice -n 10 cargo build --release --offline --bin job
nice -n 10 python3 dist/selftest.py --binary ./target/release/job
python3 -c '\''import json,platform,time; from pathlib import Path; Path("dist/ready.json").write_text(json.dumps({"ready":True,"treeProtocol":1,"machine":platform.machine(),"tested_at":time.time()}))'\''
'
echo "$host: native build and small self-test passed"
