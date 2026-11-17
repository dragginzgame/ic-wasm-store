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
mkdir -p "$fixture/server/.setup-v1-1-0"
printf 'stale failure must not be displayed\n' > "$fixture/server/.setup-v1-1-0/download.stderr"
cat > "$fixture/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf 'curl: (56) simulated receive failure\n::error::untrusted command\r\033[2J\n' >&2
printf '%05000d\n' 0 >&2
exit 56
STUB
chmod +x "$fixture/bin/curl"
status=0
PATH="$fixture/bin:$PATH" CARGO_NET_OFFLINE=true IC_TESTKIT_SERVER_DIRECTORY="$fixture/server" \
    bash scripts/prepare-server.sh > "$fixture/output" 2>&1 || status=$?
[[ "$status" != 0 ]] || { echo 'failed download became success'; exit 1; }
if grep -q 'stale failure must not be displayed' "$fixture/output"; then
    echo 'reported stale setup evidence' >&2
    exit 1
fi
rm -rf "$fixture/server/.setup-v1-1-0"
shopt -s nullglob
errors=("$fixture/server"/.setup-v1-*/download.stderr)
failures=("$fixture/server"/.setup-v1-*/failure.txt)
[[ ${#errors[@]} == 1 && ${#failures[@]} == 1 ]] || { cat "$fixture/output"; exit 1; }
[[ ! -e "$fixture/server/16.1.0-x86_64-linux/pocket-ic" && ! -e "$fixture/server/16.1.0-arm64-darwin/pocket-ic" && ! -e "$fixture/server/16.1.0-x86_64-darwin/pocket-ic" ]]
perl -e '
    use strict; use warnings;
    local $/; open my $f, "<:raw", shift or die $!; my $text = <$f>;
    die "missing useful cause" unless $text =~ /curl: \(56\) simulated receive failure/;
    die "missing evidence location" unless $text =~ /\.setup-v1-[^\n]*\/download\.stderr/;
    die "workflow command not neutralized" unless $text =~ /^  ::error::untrusted command\\x0d\\x1b\[2J$/m;
    die "raw control byte" if $text =~ /[\r\x1b]/;
    die "missing truncation notice" unless $text =~ /\[truncated;/;
    die "unbounded diagnostic" if $text =~ /0{4097}/;
' "$fixture/output"
cp "${errors[0]}" "$fixture/original"
bash scripts/show-server-setup-failure.sh "${errors[0]}" > "$fixture/rendered"
cmp "${errors[0]}" "$fixture/original"
bash scripts/show-server-setup-failure.sh "$fixture/missing" > "$fixture/missing-output"
grep -q 'No simulator setup evidence' "$fixture/missing-output"
# The reporting command must not replace a specific setup status (or run on success).
cat > "$fixture/bin/launcher" <<'STUB'
#!/usr/bin/env bash
if [[ "${1:-}" == --version ]]; then echo "ic-testkit-server 0.106.0"; exit 0; fi
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
grep -q 'Could not render setup evidence' "$fixture/report-failed"
printf 'server diagnostics: real failed download, bounded escaped output, retained evidence, missing evidence and status preservation passed\n'
