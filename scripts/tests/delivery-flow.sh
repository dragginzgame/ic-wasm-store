#!/usr/bin/env bash
# Consumer dispatch and failure ordering; no compiler, registry or simulator effects.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
mkdir -p "$root/target"
fixture=$(mktemp -d "$root/target/delivery-flow.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/bin" "$fixture/repo/scripts"
export FLOW_LOG="$fixture/calls"
cat > "$fixture/leaves.mk" <<'MAKE'
clippy-native clippy-wasm clippy-qualification toko-tooling-check:
	@echo $@ >> "$$FLOW_LOG"
	@test "$${FAIL_STAGE:-}" != "$@"
MAKE
cat > "$fixture/bin/make-child" <<'STUB'
#!/bin/bash
printf 'make %s\n' "$*" >> "$FLOW_LOG"
[[ "${FAIL_STAGE:-}" != product ]]
STUB
cat > "$fixture/bin/bash" <<'STUB'
#!/bin/bash
printf 'bash %s\n' "$*" >> "$FLOW_LOG"
STUB
# Use absolute interpreters so the substituted bash cannot invoke itself.
chmod +x "$fixture/bin/"*
for failure in '' clippy-native product; do
    : > "$FLOW_LOG"
    status=0
    PATH="$fixture/bin:$PATH" FAIL_STAGE="$failure" make -C "$root" -f Makefile -f "$fixture/leaves.mk" \
        -j4 release-verify MAKE="$fixture/bin/make-child" > "$fixture/output" 2>&1 || status=$?
    if [[ -z "$failure" ]]; then
        [[ "$status" == 0 ]]
        rg -q '^make fmt-check test doc$' "$FLOW_LOG"
        rg -q '^bash scripts/release-installed.sh$' "$FLOW_LOG"
        if rg -q 'test-publication|test-server-diagnostics|test-setup-evidence' "$FLOW_LOG"; then exit 1; fi
    else
        [[ "$status" != 0 ]]
        if rg -q '^bash ' "$FLOW_LOG"; then exit 1; fi
        if [[ "$failure" == clippy-native ]] && rg -q '^make ' "$FLOW_LOG"; then exit 1; fi
    fi
done
rm "$fixture/bin/bash"
cp "$root/scripts/test-installed.sh" "$fixture/repo/scripts/"
cat > "$fixture/repo/scripts/prepare-predecessor.sh" <<'STUB'
#!/bin/bash
echo predecessor >> "$FLOW_LOG"
[[ "${FAIL_TEST:-0}" != 2 ]] || exit 47
STUB
cat > "$fixture/bin/cargo" <<'STUB'
#!/bin/bash
printf 'cargo %s | original=%s\n' "$*" "${IC_WASM_STORE_WASM:-}" >> "$FLOW_LOG"
if [[ "$1" == build ]]; then
    printf 'wasm' > "$PWD/module.wasm"
    printf '{"reason":"compiler-artifact","target":{"name":"ic_wasm_store"},"filenames":["%s/module.wasm"]}\n' "$PWD"
elif [[ "${FAIL_TEST:-0}" == 1 ]]; then exit 43;
fi
STUB
chmod +x "$fixture/bin/cargo"
for failure in 0 1 2; do
    : > "$FLOW_LOG"
    status=0
    PATH="$fixture/bin:$PATH" IC_TESTKIT_POCKET_IC_URL=http://fixture FAIL_TEST="$failure" \
        /bin/bash "$fixture/repo/scripts/test-installed.sh" --with-predecessor > "$fixture/output" 2>&1 || status=$?
    [[ $(rg -c '^cargo build ' "$FLOW_LOG") == 2 ]]
    if [[ "$failure" == 0 ]]; then
        [[ "$status" == 0 ]]
        [[ $(rg -c '^cargo test ' "$FLOW_LOG") == 2 ]]
        rg -q 'versioned_upload_resumes.*original=.*/predecessor.wasm' "$FLOW_LOG"
    elif [[ "$failure" == 1 ]]; then
        [[ "$status" == 43 ]]
        if rg -q '^predecessor$|versioned_upload_resumes' "$FLOW_LOG"; then exit 1; fi
    else
        [[ "$status" == 47 ]]
        rg -q '^predecessor$' "$FLOW_LOG"
        if rg -q 'versioned_upload_resumes' "$FLOW_LOG"; then exit 1; fi
    fi
done
if [[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]; then
    cp "$root/scripts/release-installed.sh" "$fixture/repo/scripts/"
    printf baseline > "$fixture/repo/Cargo.lock"
    cat > "$fixture/bin/git" <<'STUB'
#!/bin/bash
[[ "$1" == ls-files ]] || exit 98
printf 'Cargo.lock\0'
STUB
    cat > "$fixture/bin/cargo" <<'STUB'
#!/bin/bash
echo changed >> Cargo.lock
if [[ "$1" == build ]]; then
    printf wasm > module.wasm
    printf '{"reason":"compiler-artifact","target":{"name":"ic_wasm_store"},"filenames":["%s/module.wasm"]}\n' "$PWD"
else
    printf '{"reason":"compiler-artifact","target":{"name":"store"},"executable":"/usr/bin/true"}\n'
fi
STUB
    cat > "$fixture/repo/scripts/testkit-launcher.sh" <<'STUB'
#!/bin/bash
printf '%s\n' /usr/bin/true
STUB
    chmod +x "$fixture/bin/git" "$fixture/bin/cargo"
    status=0
    PATH="$fixture/bin:$PATH" /bin/bash "$fixture/repo/scripts/release-installed.sh" > "$fixture/drift-output" 2>&1 || status=$?
    [[ "$status" == 2 ]]
    rg -q 'Source changed during installed preparation' "$fixture/drift-output"
    [[ ! -d "$fixture/repo/target/installed-evidence" ]]
fi
printf 'delivery flow: early lint refusal, separate tooling, shared Wasm builds and installed failure ordering passed\n'
