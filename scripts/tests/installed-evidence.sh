#!/usr/bin/env bash
# Exercise Store's real input observer with the shared runner; effects are substitutes.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    echo 'Installed evidence fixtures: reuse is Linux x86_64 only; other hosts execute normally.'
    exit 0
fi
mkdir -p "$root/target"
fixture=$(mktemp -d "$root/target/installed-evidence-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/crates" "$fixture/qualification/ic-wasm-store-qualification" "$fixture/scripts" "$fixture/target"
cp "$root/scripts/observe-installed.pl" "$fixture/scripts/"
for name in Makefile Cargo.toml Cargo.lock qualification/Cargo.toml qualification/Cargo.lock rust-toolchain.toml .toko-tooling.snapshot; do
    printf 'fixture\n' > "$fixture/$name"
done
printf original > "$fixture/target/original.wasm"
printf candidate > "$fixture/target/candidate.wasm"
cp /usr/bin/true "$fixture/target/test-binary"
cat > "$fixture/scripts/effect.sh" <<'STUB'
#!/bin/bash
echo run >> target/count
[[ ! -e target/fail ]] || exit 43
if [[ -e target/drift ]]; then echo drift >> crates/input; fi
STUB
cd "$fixture"
runner="$root/vendor/toko-tooling/scripts/ci/run-validation-stage.pl"
jq -cnS --arg binary "$fixture/target/test-binary" --arg original "$fixture/target/original.wasm" --arg candidate "$fixture/target/candidate.wasm" \
    '{PATH:"/usr/bin:/bin",LANG:"C",LC_ALL:"C",TZ:"UTC",STORE_TEST_BINARY:$binary,STORE_TEST_LAUNCHER:"/usr/bin/true",STORE_TEST_SERVER:"/usr/bin/true",IC_WASM_STORE_WASM:$original,IC_WASM_STORE_CANDIDATE:$candidate}' > target/environment.json
run() {
    perl "$runner" --state target/proofs --stage current --contract store-installed-binaries/1 \
        --source 0000000000000000000000000000000000000001 --group store \
        --observe scripts/observe-installed.pl --environment target/environment.json \
        --max-bytes 536870912 -- /bin/bash scripts/effect.sh > target/output 2>&1
}
expect_mode() { run || { cat target/output; exit 1; }; rg -q "Validation stage current: $1;" target/output; }
expect_mode executed
expect_mode reused
[[ $(wc -l < target/count) == 1 ]]
mv crates target/saved-crates
if run; then echo 'Missing required source root admitted' >&2; exit 1; fi
[[ $(wc -l < target/count) == 1 ]]
mv target/saved-crates crates
# Recompiled artifact, source inventory and environment changes invalidate evidence.
printf changed >> target/candidate.wasm
expect_mode executed
printf new > crates/new.rs
expect_mode executed
jq -cS '.TZ="Europe/Monaco"' target/environment.json > target/next.json
mv target/next.json target/environment.json
expect_mode executed
# Tampered original execution logs must never authorize reuse.
for proof in target/proofs/proofs/*.json; do
    log=$(jq -er '.logs.stdout.path' "$proof")
    printf tampered >> "$log"
done
expect_mode executed
# Missing proofs execute normally; retain earlier evidence under another fixture path.
mv target/proofs target/old-proofs
expect_mode executed
# A failed child with plausible output creates no reusable proof; retry executes.
printf failure >> target/original.wasm
touch target/fail
status=0
run || status=$?
[[ "$status" == 43 ]]
rm target/fail
expect_mode executed
expect_mode reused
# Source drift during execution refuses admission and must rerun after repair.
printf next >> target/original.wasm
touch target/drift
status=0
run || status=$?
[[ "$status" != 0 ]]
rg -q 'inputs changed during execution' target/output
rm target/drift
expect_mode executed
# Unsafe evidence routes refuse rather than following caller-selected links.
mv target/proofs target/saved-proofs
ln -s saved-proofs target/proofs
status=0
run || status=$?
[[ "$status" != 0 ]]
rg -q 'symlink' target/output
printf 'installed evidence: reuse, artifact/source/environment invalidation, tampered/missing proof, failure recovery and input drift passed\n'
