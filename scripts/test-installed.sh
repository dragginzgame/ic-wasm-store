#!/usr/bin/env bash
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
with_predecessor=false
build_only=false
if [[ "${1:-}" == --build-only ]]; then
    build_only=true
    shift
    [[ $# == 0 ]] || { echo '--build-only takes no test filters' >&2; exit 2; }
fi
if [[ "${1:-}" == --with-predecessor ]]; then
    with_predecessor=true
    shift
    [[ $# == 0 ]] || { echo '--with-predecessor requires the complete installed suite' >&2; exit 2; }
fi
mkdir -p target/qualification
for variant in original candidate; do
    features=()
    if [[ "$variant" == candidate ]]; then features=(--features maintenance-candidate); fi
    cargo build --locked --release --target wasm32-unknown-unknown -p ic-wasm-store "${features[@]+"${features[@]}"}" --message-format=json-render-diagnostics > "target/qualification/$variant.jsonl"
    wasm=$(jq -sr '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_wasm_store") | .filenames[] | select(endswith(".wasm"))] | unique | if length == 1 then .[0] else error("expected Store Wasm") end' "target/qualification/$variant.jsonl")
    cp "$wasm" "target/qualification/$variant.wasm"
done
if [[ "$build_only" == true ]]; then exit 0; fi
export IC_WASM_STORE_WASM="${IC_WASM_STORE_WASM:-$root/target/qualification/original.wasm}"
export IC_WASM_STORE_CANDIDATE="$root/target/qualification/candidate.wasm"
command=(cargo test --locked --manifest-path qualification/Cargo.toml --test store -- "$@" --nocapture)
run_tests() {
    if [[ -n "${IC_TESTKIT_POCKET_IC_URL:-}" ]]; then
        "${command[@]}"
    else
        local launcher
        launcher=$(bash scripts/testkit-launcher.sh check)
        bash scripts/run-server.sh "$launcher" "${IC_TESTKIT_SERVER_DIRECTORY:-$root/target/qualification/server-0.110.5}" \
            "${command[@]}"
    fi
}
run_tests
if [[ "$with_predecessor" == true ]]; then
    bash scripts/prepare-predecessor.sh build
    export IC_WASM_STORE_WASM="$root/target/qualification/predecessor.wasm"
    command=(cargo test --locked --manifest-path qualification/Cargo.toml --test store -- predecessor_image_is_rejected --ignored --nocapture)
    run_tests
fi
