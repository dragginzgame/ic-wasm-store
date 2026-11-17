#!/usr/bin/env bash
# Shared tooling owns proofs. Store selects two exact installed-test stages.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
# Custom runtime selection and unqualified hosts execute the complete ordinary path.
reason=''
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || reason='host is not qualified for reuse'
for name in IC_TESTKIT_POCKET_IC_URL IC_TESTKIT_SERVER_DIRECTORY TESTKIT_RELEASE_LAUNCHER POCKET_IC_BIN IC_WASM_STORE_WASM IC_WASM_STORE_CANDIDATE LD_PRELOAD LD_LIBRARY_PATH RUST_TEST_THREADS RUST_TEST_TIME_UNIT RUST_LOG RUST_BACKTRACE; do
    if printenv "$name" > /dev/null; then reason="explicit $name selection requires ordinary execution"; fi
done
if [[ -n "$reason" ]]; then
    printf 'Installed qualification: executing; %s\n' "$reason"
    exec bash scripts/test-installed.sh --with-predecessor
fi
mkdir -p target/qualification
lock="$root/target/qualification/.release-installed-lock"
mkdir "$lock" || { echo 'Installed qualification is occupied; inspect the existing attempt before recovery.' >&2; exit 2; }
trap 'rmdir "$lock"' EXIT
source_identity() {
    git ls-files -z --cached --others --exclude-standard | sort -zu | xargs -0 sha256sum | sha256sum | cut -d ' ' -f 1
}
source_commit=$(source_identity)
check_source() {
    [[ $(source_identity) == "$source_commit" ]] || { echo 'Source changed during installed preparation/admission; no complete gate admitted.' >&2; exit 2; }
}
# Compile and admit actual final artifacts on every call. Proofs cover execution only.
bash scripts/test-installed.sh --build-only
bash scripts/prepare-predecessor.sh build
cargo test --locked --manifest-path qualification/Cargo.toml --test store --no-run --message-format=json-render-diagnostics > target/qualification/installed-test.jsonl
binary=$(jq -ser '[.[] | select(.reason == "compiler-artifact" and .target.name == "store" and .executable != null) | .executable] | unique | if length == 1 then .[0] else error("expected installed test executable") end' target/qualification/installed-test.jsonl)
launcher=$(bash scripts/testkit-launcher.sh check)
server=$("$launcher" check --directory "$root/target/qualification/server")
# Bind dirty development checks as well as committed delivery to actual source bytes.
# Final Git/clean-source admission remains the publication adapter's obligation.
check_source
mkdir -p target/installed-evidence
attempt=$(mktemp -d "$root/target/installed-evidence/selection.XXXXXX")
# One fixed runtime environment: local disposable server, no ambient configuration.
for stage in current predecessor; do
    check_source
    original="$root/target/qualification/original.wasm"
    arguments=()
    if [[ "$stage" == predecessor ]]; then
        original="$root/target/qualification/predecessor.wasm"
        arguments=(versioned_upload_resumes)
    fi
    jq -cnS --arg binary "$binary" --arg launcher "$launcher" --arg server "$server" \
        --arg original "$original" --arg candidate "$root/target/qualification/candidate.wasm" \
        '{PATH:"/usr/bin:/bin",LANG:"C",LC_ALL:"C",TZ:"UTC",STORE_TEST_BINARY:$binary,STORE_TEST_LAUNCHER:$launcher,STORE_TEST_SERVER:$server,IC_WASM_STORE_WASM:$original,IC_WASM_STORE_CANDIDATE:$candidate}' > "$attempt/$stage-environment.json"
    perl vendor/toko-tooling/scripts/ci/run-validation-stage.pl \
        --state target/installed-evidence/stages --stage "$stage" --contract store-installed-binaries/1 \
        --source "$source_commit" --group store --observe scripts/observe-installed.pl \
        --environment "$attempt/$stage-environment.json" --max-bytes 536870912 \
        --admission-output "$attempt/$stage-admission.json" -- \
        "$launcher" run --directory "$root/target/qualification/server" --ttl 1800 -- \
        "$binary" "${arguments[@]+"${arguments[@]}"}" --nocapture
    check_source
done
