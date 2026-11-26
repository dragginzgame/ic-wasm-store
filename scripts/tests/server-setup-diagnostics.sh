#!/usr/bin/env bash
# Real Testkit failed-download path; only curl is substituted, with no networking.
set -euo pipefail
# Exercise relative setup entrypoints with the caller's CDPATH enabled.
export CDPATH=.
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
mkdir -p target
fixture=$(mktemp -d "$root/target/server-diagnostics.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/bin"
mkdir -p "$fixture/retained"
mkdir -p "$fixture/server/.setup-v1-1-0"
printf 'stale failure must not be displayed\n' > "$fixture/server/.setup-v1-1-0/download.stderr"
cat > "$fixture/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf 'curl: (56) simulated receive failure\n::error::untrusted command\r\033[2J\n' >&2
printf 'details Bearer standalone-fixture-value\n{"payload":"standalone-body-value"}\n' >&2
printf '%010000d\n' 0 >&2
exit 56
STUB
chmod +x "$fixture/bin/curl"
status=0
PATH="$fixture/bin:$PATH" IC_TESTKIT_SERVER_DIRECTORY="$fixture/server" \
    STORE_SETUP_EVIDENCE="$fixture/retained" \
    bash scripts/prepare-server.sh > "$fixture/output" 2>&1 || status=$?
[[ "$status" != 0 ]] || { echo 'failed download became success'; exit 1; }
if grep -q 'stale failure must not be displayed' "$fixture/output"; then
    echo 'reported stale setup evidence' >&2
    exit 1
fi
rm -rf "$fixture/server/.setup-v1-1-0"
evidence=$(sed -n 's/^ic-testkit-server setup failed (exit 56); evidence: //p' "$fixture/output")
errors=("$evidence/launcher/download.stderr")
[[ -f "${errors[0]}" ]]
rg -q 'Source: .*launcher/download.stderr' "$fixture/retained"
rg -q 'curl: \(56\) simulated receive failure' "$fixture/retained"
if rg -q 'stale failure must not be displayed' "$fixture/retained"; then exit 1; fi
[[ ! -e "$fixture/server/16.1.0-x86_64-linux/pocket-ic" && ! -e "$fixture/server/16.1.0-arm64-darwin/pocket-ic" && ! -e "$fixture/server/16.1.0-x86_64-darwin/pocket-ic" ]]
perl -e '
    use strict; use warnings;
    local $/; open my $f, "<:raw", shift or die $!; my $text = <$f>;
    die "missing useful cause" unless $text =~ /curl: \(56\) simulated receive failure/;
    die "missing evidence location" unless $text =~ /launcher\/download\.stderr/;
    die "workflow command not neutralized" unless $text =~ /^  : :error: :untrusted command/m;
    die "raw control byte" if $text =~ /[\r\x1b]/;
    die "missing truncation notice" unless $text =~ /oversized lines omitted/;
    die "unbounded diagnostic" if $text =~ /0{4097}/;
' "$fixture/output"
cp "${errors[0]}" "$fixture/original"
perl scripts/collect-setup-evidence.pl --console-only "${errors[0]}" > "$fixture/rendered"
cmp "${errors[0]}" "$fixture/original"
rg -q 'curl: \(56\) simulated receive failure' "$fixture/rendered"
if rg -q 'standalone-fixture-value|standalone-body-value' "$fixture/output" "$fixture/retained" "$fixture/rendered"; then
    echo 'sensitive value escaped simulator diagnostics' >&2
    exit 1
fi
# The reporting command must not replace a specific setup status (or run on success).
cat > "$fixture/bin/launcher" <<'STUB'
#!/usr/bin/env bash
if [[ "${1:-}" == --version ]]; then echo "ic-testkit-server 0.110.5"; exit 0; fi
if [[ "${1:-}" == check ]]; then exit "${CHECK_STATUS:-0}"; fi
exit "${SETUP_STATUS:?}"
STUB
chmod +x "$fixture/bin/launcher"
for expected in 0 37; do
    status=0
    TESTKIT_RELEASE_LAUNCHER="$fixture/bin/launcher" PATH="$fixture/bin:$PATH" SETUP_STATUS="$expected" IC_TESTKIT_SERVER_DIRECTORY="$fixture/missing" \
        bash scripts/prepare-server.sh > "$fixture/status-output" 2>&1 || status=$?
    [[ "$status" == "$expected" ]]
    if (( expected == 0 )); then [[ ! -s "$fixture/status-output" ]]; fi
done
# Successful provisioning must not hide a failed independent offline admission.
status=0
TESTKIT_RELEASE_LAUNCHER="$fixture/bin/launcher" SETUP_STATUS=0 CHECK_STATUS=39 \
    IC_TESTKIT_SERVER_DIRECTORY="$fixture/missing" \
    bash scripts/prepare-server.sh > "$fixture/check-failed" 2>&1 || status=$?
[[ "$status" == 39 ]]
grep -q 'check failed (exit 39); evidence:' "$fixture/check-failed"
cat > "$fixture/bin/perl" <<'STUB'
#!/usr/bin/env bash
exit 9
STUB
chmod +x "$fixture/bin/perl"
status=0
TESTKIT_RELEASE_LAUNCHER="$fixture/bin/launcher" PATH="$fixture/bin:$PATH" SETUP_STATUS=37 IC_TESTKIT_SERVER_DIRECTORY="$fixture/server" \
    bash scripts/prepare-server.sh > "$fixture/report-failed" 2>&1 || status=$?
[[ "$status" == 37 ]]
grep -q 'Could not collect simulator diagnostics' "$fixture/report-failed"
printf 'server diagnostics: real failed download, bounded sanitized output, retained evidence and status preservation passed\n'
