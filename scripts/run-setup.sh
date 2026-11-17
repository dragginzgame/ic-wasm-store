#!/usr/bin/env bash
# Keep setup's exact status; publish sanitized text rather than raw installer output.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
phase=${1:-}
[[ "$phase" =~ ^[a-z][a-z0-9-]*$ && $# -ge 2 ]] || {
    echo 'usage: run-setup.sh PHASE COMMAND [ARG...]' >&2; exit 2;
}
shift
for directory in target target/setup-raw target/setup-evidence; do
    [[ ! -L "$directory" && ( ! -e "$directory" || -d "$directory" ) ]] || {
        echo 'Setup evidence requires physical directories inside the repository.' >&2; exit 2;
    }
    mkdir -p "$directory"
done
raw=$(mktemp -d "$root/target/setup-raw/$phase.XXXXXX")
export STORE_SETUP_EVIDENCE
STORE_SETUP_EVIDENCE=$(mktemp -d "$root/target/setup-evidence/$phase.XXXXXX")
status=0
reporting_complete=false
# shellcheck disable=SC2329 # Invoked by the EXIT trap below.
finish() {
    local report_status=$?
    trap - EXIT
    if [[ "$reporting_complete" != true ]]; then
        echo "Setup evidence collection interrupted; setup status: $status. Inspect collection errors above." >&2 || :
    fi
    if (( status != 0 )); then exit "$status"; fi
    exit "$report_status"
}
trap finish EXIT
"$@" > "$raw/stdout" 2> "$raw/stderr" || status=$?
printf 'phase=%s\nstatus=%s\nRaw logs omitted; artifacts contain bounded sanitized text only.\n' \
    "$phase" "$status" > "$STORE_SETUP_EVIDENCE/result.txt"
collection_status=0
perl scripts/collect-setup-evidence.pl "$STORE_SETUP_EVIDENCE" "$raw/stdout" "$raw/stderr" || collection_status=$?
printf 'collection_status=%s\n' "$collection_status" >> "$STORE_SETUP_EVIDENCE/result.txt"
printf 'Setup phase %s: exit %s; evidence: %s\n' "$phase" "$status" "$STORE_SETUP_EVIDENCE"
if (( collection_status != 0 )); then
    echo 'Setup evidence collection failed; inspect the collection error above.' >&2
fi
reporting_complete=true
if (( status != 0 )); then exit "$status"; fi
exit "$collection_status"
