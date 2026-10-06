#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if ! node --input-type=module -e 'import {original} from "./test/original-helper.js";process.exit(original?0:77)'; then
  echo 'SKIP: optional original unavailable'; exit 0
fi
export CARGO_TARGET_DIR="$(pwd)/rust/target"
cargo build --release --offline --manifest-path rust/Cargo.toml --bin perft --bin crosscheck
for depth in 0 1 2 3; do
 native=$(rust/target/release/perft "$depth")
 reference=$(node rust/tools/perft.mjs "$depth")
 node --input-type=module -e 'const a=JSON.parse(process.argv[1]),b=JSON.parse(process.argv[2]);if(a.leaves!==b.leaves)throw Error("perft mismatch");console.log("PASS perft",a.depth,a.leaves)' "$native" "$reference"
done
node rust/tools/generate.mjs "${1:-2000}" | rust/target/release/crosscheck
