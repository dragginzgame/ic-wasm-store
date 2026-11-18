#!/usr/bin/env bash
# Qualify consumer admission ordering without registry writes or hosted API calls.
set -euo pipefail
# Relative invocations must not capture cd's CDPATH-induced stdout.
export CDPATH=.
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
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
# Hosted status must not be queried or required by archive admission.
touch "$FIXTURE/hosted-query"
exit 97
STUB
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -eu
case "$*" in
    'metadata --locked --no-deps --format-version 1')
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
for CASE in success dirty status_failed package_failed provenance_failed changed dirty_after; do
    export CASE
    rm -f "$fixture/packaged" "$fixture/provenance"
    status=0
    (cd "$fixture" && CARGO_TARGET_DIR="$fixture/other-target" bash scripts/check-publication.sh) > "$fixture/output" 2>&1 || status=$?
    [[ ! -e "$fixture/hosted-query" ]] || { echo 'archive admission queried hosted CI'; exit 1; }
    if [[ "$CASE" == success ]]; then
        [[ "$status" == 0 && -f "$fixture/provenance" ]] || { cat "$fixture/output"; exit 1; }
    else
        [[ "$status" != 0 ]] || { echo "incorrect admission: $CASE"; exit 1; }
        if [[ "$CASE" != changed && "$CASE" != dirty_after && "$CASE" != provenance_failed ]]; then [[ ! -f "$fixture/packaged" ]]; fi
        if [[ "$CASE" != changed && "$CASE" != dirty_after ]]; then [[ ! -f "$fixture/provenance" ]]; fi
    fi
    printf 'publication admission: %s passed\n' "$CASE"
done
