#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 2 ]]; then
    echo 'usage: scripts/write-wire-api.sh RELEASE_WASM EMPTY_OUTPUT_DIRECTORY' >&2
    exit 2
fi
wasm=$1
output=$2
[[ $wasm = /* ]] || wasm="$PWD/$wasm"
[[ $output = /* ]] || output="$PWD/$output"
STORE_WIRE_API_WASM="$wasm" STORE_WIRE_API_OUTPUT="$output" \
    cargo test --locked -p ic-wasm-store --lib -- canister::wire_api::write_manifests --exact --ignored
