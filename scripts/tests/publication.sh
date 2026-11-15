#!/usr/bin/env bash
# Qualify consumer admission ordering without registry writes or hosted API calls.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/scripts" "$fixture/bin" "$fixture/vendor/toko-tooling/scripts/ci"
cp "$root/scripts/check-publication.sh" "$fixture/scripts/"
export FIXTURE="$fixture"
export PATH="$fixture/bin:$PATH"
cat > "$fixture/bin/git" <<'STUB'
#!/usr/bin/env bash
set -eu
case "$*" in
    'status --porcelain --untracked-files=all')
        [[ "$CASE" != status_failed ]] || exit 1
        if [[ "$CASE" == dirty || ( "$CASE" == dirty_after && -f "$FIXTURE/packaged" ) ]]; then
            echo ' M Cargo.toml'
        fi ;;
    'rev-parse HEAD')
        if [[ "$CASE" == changed && -f "$FIXTURE/packaged" ]]; then
            printf '%040d\n' 2
        else
            printf '%040d\n' 1
        fi ;;
    *) exit 98 ;;
esac
STUB
cat > "$fixture/bin/gh" <<'STUB'
#!/usr/bin/env bash
set -eu
[[ "$*" == "api --method GET repos/dragginzgame/ic-wasm-store/actions/workflows/ci.yml/runs -f head_sha=0000000000000000000000000000000000000001 -f event=push -f per_page=100" ]]
[[ "$CASE" != api_failed ]] || exit 1
cat "$FIXTURE/runs.json"
STUB
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -eu
case "$*" in
    'metadata --locked --offline --no-deps --format-version 1')
        echo '{"packages":[{"name":"ic-wasm-store","version":"0.2.3"},{"name":"ic-wasm-store-protocol","version":"0.2.3"}]}' ;;
    "package --locked --target-dir $FIXTURE/target --target wasm32-unknown-unknown -p ic-wasm-store-protocol -p ic-wasm-store")
        [[ "$CASE" != package_failed ]] || exit 1
        touch "$FIXTURE/packaged" ;;
    *) exit 98 ;;
esac
STUB
cat > "$fixture/vendor/toko-tooling/scripts/ci/check-cargo-package-provenance.sh" <<'STUB'
#!/usr/bin/env bash
set -eu
[[ "$2" == "$FIXTURE/target/package" && "$3" == https://github.com/dragginzgame/ic-wasm-store.git && "$4" == 0000000000000000000000000000000000000001 ]]
shift 4
[[ "$*" == '--package ic-wasm-store-protocol@0.2.3 crates/ic-wasm-store-protocol --package ic-wasm-store@0.2.3 crates/ic-wasm-store' ]]
[[ "$CASE" != provenance_failed ]] || exit 1
touch "$FIXTURE/provenance"
STUB
chmod +x "$fixture/bin/"*
for CASE in success dirty status_failed absent pending failed wrong_commit wrong_branch latest_failed api_failed package_failed provenance_failed changed dirty_after; do
    export CASE
    rm -f "$fixture/packaged" "$fixture/provenance"
    jq -n --arg case "$CASE" '
        {head_sha:"0000000000000000000000000000000000000001", event:"push", head_branch:"main", run_number:1, run_attempt:1, status:"completed", conclusion:"success"} |
        (if $case == "pending" then .status = "in_progress"
         elif $case == "failed" then .conclusion = "failure"
         elif $case == "wrong_commit" then .head_sha = "other"
         elif $case == "wrong_branch" then .head_branch = "other"
         else . end) |
        {workflow_runs: (if $case == "absent" then []
         elif $case == "latest_failed" then [., (. + {run_attempt:2, conclusion:"failure"})]
         else [.] end)}' > "$fixture/runs.json"
    status=0
    CARGO_TARGET_DIR="$fixture/other-target" bash "$fixture/scripts/check-publication.sh" > "$fixture/output" 2>&1 || status=$?
    if [[ "$CASE" == success ]]; then
        [[ "$status" == 0 && -f "$fixture/provenance" ]] || { cat "$fixture/output"; exit 1; }
    else
        [[ "$status" != 0 ]] || { echo "incorrect admission: $CASE"; exit 1; }
        if [[ "$CASE" != changed && "$CASE" != dirty_after && "$CASE" != provenance_failed ]]; then [[ ! -f "$fixture/packaged" ]]; fi
        if [[ "$CASE" != changed && "$CASE" != dirty_after ]]; then [[ ! -f "$fixture/provenance" ]]; fi
    fi
    printf 'publication admission: %s passed\n' "$CASE"
done
