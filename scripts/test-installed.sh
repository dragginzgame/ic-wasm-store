#!/usr/bin/env bash
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
mkdir -p target/qualification
for variant in original candidate; do
    features=()
    if [[ "$variant" == candidate ]]; then features=(--features maintenance-candidate); fi
    cargo build --locked --release --target wasm32-unknown-unknown -p ic-wasm-store "${features[@]+"${features[@]}"}" --message-format=json-render-diagnostics > "target/qualification/$variant.jsonl"
    wasm=$(jq -sr '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_wasm_store") | .filenames[] | select(endswith(".wasm"))] | unique | if length == 1 then .[0] else error("expected Store Wasm") end' "target/qualification/$variant.jsonl")
    cp "$wasm" "target/qualification/$variant.wasm"
done
export IC_WASM_STORE_WASM="${IC_WASM_STORE_WASM:-$root/target/qualification/original.wasm}"
export IC_WASM_STORE_CANDIDATE="$root/target/qualification/candidate.wasm"
command=(cargo test --locked --manifest-path qualification/Cargo.toml --test store -- "$@" --nocapture)
if [[ -n "${IC_TESTKIT_POCKET_IC_URL:-}" ]]; then
    exec "${command[@]}"
fi
launcher=${TESTKIT_RELEASE_LAUNCHER:-$root/.tools/testkit-bootstrap/bin/ic-testkit-server}
[[ "$("$launcher" --version)" == 'ic-testkit-server 0.106.0' ]] || {
    echo 'Run make qualification-setup to prepare the reviewed server launcher.' >&2
    exit 2
}
exec "$launcher" run --directory "${IC_TESTKIT_SERVER_DIRECTORY:-$root/target/qualification/server}" \
    --ttl 1800 -- "${command[@]}"
