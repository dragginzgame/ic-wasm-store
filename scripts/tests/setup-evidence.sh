#!/usr/bin/env bash
# Offline failures through the actual CI wrapper and selected-tool caller.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
mkdir -p "$root/target"
fixture=$(mktemp -d "$root/target/setup-evidence-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
repo="$fixture/repo with spaces"
mkdir -p "$repo/scripts" "$repo/vendor/toko-tooling/scripts/"{ci,dev} "$fixture/bin"
cp "$root/scripts/run-setup.sh" "$root/scripts/collect-setup-evidence.pl" "$root/scripts/testkit-launcher.sh" "$repo/scripts/"
cp "$root/vendor/toko-tooling/scripts/ci/SetupDiagnostics.pm" "$repo/vendor/toko-tooling/scripts/ci/"
cp "$root/vendor/toko-tooling/scripts/dev/prepare-rust-toolchain.sh" "$repo/vendor/toko-tooling/scripts/dev/"
cp "$root/rust-toolchain.toml" "$repo/"
cd "$repo"
export CDPATH=.
export FIXTURE_TOKEN=fixture-sensitive-value
export FIXTURE_ACCESS_KEY=fixture-access-value

# Run the selected workflow step with the real installer and a substitute rustup.
compiler_setup=$(yq -o=json '.jobs.qualify.steps' "$root/.github/workflows/ci.yml" |
    jq -er '[.[] | select(.id == "compiler") | .run] | if length == 1 then .[0] else error("expected one compiler step") end | select(type == "string" and length > 0)')
export COMPILER_CALLS="$fixture/compiler-calls"
cat > "$fixture/bin/rustup" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$COMPILER_CALLS"
if [[ "${COMPILER_STATUS:-0}" != 0 ]]; then
    printf 'rustup fixture could not install the compiler\nAuthorization: Bearer compiler-fixture-value\n' >&2
    exit "$COMPILER_STATUS"
fi
STUB
chmod +x "$fixture/bin/rustup"
PATH="$fixture/bin:$PATH" bash -ec "$compiler_setup" > "$fixture/compiler-success" 2>&1
printf '%s\n' toolchain install 1.100.0 --profile minimal --component rustfmt --component clippy \
    --target wasm32-unknown-unknown > "$fixture/compiler-expected"
cmp "$COMPILER_CALLS" "$fixture/compiler-expected"
status=0
PATH="$fixture/bin:$PATH" COMPILER_STATUS=52 bash -ec "$compiler_setup" > "$fixture/compiler-failure" 2>&1 || status=$?
[[ "$status" == 52 ]]
rg -q '^status=52$' target/setup-evidence/compiler.*/result.txt
rg -q 'rustup fixture could not install the compiler' "$fixture/compiler-failure"
rg -q 'rustup fixture could not install the compiler' target/setup-evidence/compiler.*/diagnostic-*
if rg -q 'compiler-fixture-value' "$fixture/compiler-failure" target/setup-evidence/compiler.*; then exit 1; fi

cat > "$fixture/fail" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p target/server/.setup-v1-1-2
printf 'curl: (56) simulated receive failure\nAuthorization: Bearer hidden-secret\n' > target/server/.setup-v1-1-2/download.stderr
perl scripts/collect-setup-evidence.pl "$STORE_SETUP_EVIDENCE" target/server/.setup-v1-1-2/download.stderr
printf 'compiler/scanner setup refused\n::error::untrusted\r\033[2J\n' >&2
printf 'token=%s\nhttps://user:secret@example.test/log?key=hidden-query\n' "$FIXTURE_TOKEN" >&2
printf 'received Bearer bare-fixture-value\n{"payload":"fixture-body-value"}\ncontext %s\n' "$FIXTURE_ACCESS_KEY" >&2
for i in {1..30}; do printf 'ordinary context line %03d%200s\n' "$i" '' >&2; done
exit 37
STUB
for phase in gitleaks compiler; do
    status=0
    bash scripts/run-setup.sh "$phase" bash "$fixture/fail" > "$fixture/$phase-output" 2>&1 || status=$?
    [[ "$status" == 37 ]]
    rg -q 'compiler/scanner setup refused' "$fixture/$phase-output"
    rg -q 'curl: \(56\) simulated receive failure' "$fixture/$phase-output"
    rg -q '^  : :error: :untrusted' "$fixture/$phase-output"
    rg -q 'console excerpt truncated' "$fixture/$phase-output"
done
# The scanner's actual shared installer must preserve its failed downloader status.
cat > "$fixture/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf 'curl: (56) scanner download refused\ntoken=hidden-scanner-token\n' >&2
exit 56
STUB
chmod +x "$fixture/bin/curl"
status=0
PATH="$fixture/bin:$PATH" TOKO_TOOLING_DOWNLOAD_CACHE='' bash scripts/run-setup.sh scanner-download \
    bash "$root/vendor/toko-tooling/scripts/ci/install-ci-tool.sh" gitleaks \
    --install-dir "$fixture/ci-tools" --version 1.2.3 \
    --sha256 0000000000000000000000000000000000000000000000000000000000000000 \
    > "$fixture/scanner-download" 2>&1 || status=$?
[[ "$status" == 56 ]]
rg -q 'curl: \(56\) scanner download refused' "$fixture/scanner-download"
if rg -q 'hidden-scanner-token' "$fixture/scanner-download" target/setup-evidence; then exit 1; fi
rg -q 'status=37' target/setup-evidence/*/result.txt
rg -q 'curl: \(56\) simulated receive failure' target/setup-evidence/*/diagnostic-*
if rg -q 'fixture-sensitive-value|hidden-secret|user:secret|hidden-query|bare-fixture-value|fixture-body-value|fixture-access-value' \
    target/setup-evidence "$fixture/gitleaks-output" "$fixture/compiler-output"; then
    echo 'sensitive value escaped sanitization' >&2; exit 1
fi
perl -e '
    for my $file (@ARGV) {
        -s $file <= 32768 or die "unbounded artifact\n";
        open my $f, "<:raw", $file or die $!; local $/; my $text = <$f>;
        $text !~ /[\r\x1b]/ or die "raw control byte\n";
    }
' target/setup-evidence/*/diagnostic-*
# Oversized logs are bounded before publication; missing/linked logs fail closed.
mkdir -p target/collected
perl -e 'print "x" x 2000000' > target/large
perl scripts/collect-setup-evidence.pl target/collected target/large > "$fixture/large-output"
rg -q 'input truncated at byte limit' target/collected/*
ln -s "$fixture/fail" target/linked
ln -s "$fixture" target/linked-directory
for file in target/missing target/linked target/linked-directory/fail; do
    status=0
    perl scripts/collect-setup-evidence.pl target/collected "$file" > "$fixture/rejected" 2>&1 || status=$?
    [[ "$status" == 1 ]]
    rg -q 'Evidence collection failed' "$fixture/rejected"
done
# Refuse redirected artifact directories before running setup or writing metadata.
mkdir -p "$fixture/redirected/scripts" "$fixture/outside"
cp scripts/run-setup.sh "$fixture/redirected/scripts/"
ln -s "$fixture/outside" "$fixture/redirected/target"
status=0
bash "$fixture/redirected/scripts/run-setup.sh" linked-target touch "$fixture/should-not-run" \
    > "$fixture/linked-target-output" 2>&1 || status=$?
[[ "$status" == 2 && ! -e "$fixture/should-not-run" ]]
[[ -z "$(ls -A "$fixture/outside")" ]]
# Even failure to write the metadata must retain the original command's status.
status=0
bash scripts/run-setup.sh missing-directory bash -c 'rmdir "$STORE_SETUP_EVIDENCE"; exit 47' \
    > "$fixture/metadata-output" 2>&1 || status=$?
[[ "$status" == 47 ]]
rg -q 'Setup evidence collection interrupted' "$fixture/metadata-output"
# Collector failure cannot hide the original setup status or make success green.
printf '#!/usr/bin/env bash\nexit 9\n' > "$fixture/bin/perl"
chmod +x "$fixture/bin/perl"
for expected in 0 43; do
    status=0
    PATH="$fixture/bin:$PATH" bash scripts/run-setup.sh failed-collector bash -c "exit $expected" > "$fixture/collector-output" 2>&1 || status=$?
    if (( expected == 0 )); then [[ "$status" == 9 ]]; else [[ "$status" == "$expected" ]]; fi
    rg -q 'Setup evidence collection failed' "$fixture/collector-output"
done
# Exercise shared-installer arguments, offline admission and the external override.
export CALLS="$fixture/installer-calls" MARKER="$fixture/probed" LAUNCHER="$fixture/launcher"
cat > "$LAUNCHER" <<'STUB'
#!/usr/bin/env bash
printf 'probed\n' >> "$MARKER"
[[ "$1" == --version ]]
echo 'ic-testkit-server 0.110.5'
STUB
chmod +x "$LAUNCHER"
cat > vendor/toko-tooling/scripts/dev/install-rust-tools.sh <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$@" > "$CALLS"
printf '%s\n' "$LAUNCHER"
exit "${INSTALL_STATUS:-0}"
STUB
for mode in setup check; do
    selected=$(bash scripts/testkit-launcher.sh "$mode")
    [[ "$selected" == "$LAUNCHER" ]]
    printf '%s\n' --consumer "$repo" --package ic-testkit --version 0.110.5 \
        --bin ic-testkit-server --profile release --features cli > "$fixture/expected-calls"
    if [[ "$mode" == check ]]; then printf '%s\n' --check >> "$fixture/expected-calls"; fi
    cmp "$CALLS" "$fixture/expected-calls"
done
rm "$MARKER"
status=0
INSTALL_STATUS=43 bash scripts/testkit-launcher.sh check > "$fixture/rejected-selection" 2>&1 || status=$?
[[ "$status" == 43 && ! -e "$MARKER" ]]
rm "$CALLS"
TESTKIT_RELEASE_LAUNCHER="$LAUNCHER" INSTALL_STATUS=43 bash scripts/testkit-launcher.sh check > "$fixture/external-selection"
[[ ! -e "$CALLS" && -e "$MARKER" ]]
printf 'setup evidence: compiler selection, scanner/compiler failures, hidden logs, sanitization, bounds, missing/linked files, collector status and launcher selection passed\n'
