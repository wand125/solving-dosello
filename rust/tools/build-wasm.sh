#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../.."
export CARGO_TARGET_DIR="$(pwd)/rust/target"
export RUSTFLAGS="--remap-path-prefix=$(pwd)=/source --remap-path-prefix=$HOME=/build"
cargo build --profile wasm --offline --manifest-path rust/Cargo.toml --target wasm32-unknown-unknown --lib
cp rust/target/wasm32-unknown-unknown/wasm/dosello_ai.wasm docs/play/wasm/dosello_ai.wasm
cp rust/data/eval3-r2.bin docs/play/wasm/eval3-r2.bin
cp engine/rules.js docs/play/engine/rules.js
node --test test/demo.test.js test/eval3.test.js
