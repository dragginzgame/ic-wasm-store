#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
mkdir -p target/qualification
for variant in original candidate; do
    features=()
    if [[ "$variant" == candidate ]]; then features=(--features maintenance-candidate); fi
    cargo build --locked --release --target wasm32-unknown-unknown -p ic-wasm-store "${features[@]+"${features[@]}"}" --message-format=json-render-diagnostics > "target/qualification/$variant.jsonl"
    wasm=$(jq -sr '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_wasm_store") | .filenames[] | select(endswith(".wasm"))] | unique | if length == 1 then .[0] else error("expected Store Wasm") end' "target/qualification/$variant.jsonl")
    cp "$wasm" "target/qualification/$variant.wasm"
done
IC_WASM_STORE_WASM="${IC_WASM_STORE_WASM:-$root/target/qualification/original.wasm}" IC_WASM_STORE_CANDIDATE="$root/target/qualification/candidate.wasm" cargo test --locked --manifest-path qualification/Cargo.toml --test store -- "$@" --nocapture
