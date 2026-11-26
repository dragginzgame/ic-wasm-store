#!/usr/bin/env bash
# The frozen launcher owns provisioning; the current Testkit library owns no server.
set -euo pipefail
umask 077
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
status=0
launcher=$(bash scripts/testkit-launcher.sh check)
directory=${IC_TESTKIT_SERVER_DIRECTORY:-$root/target/qualification/server-0.110.5}
[[ "$directory" == /* ]] || directory="$root/$directory"
directory=${directory%/}
mkdir -p target/qualification/setup-evidence
evidence=$(mktemp -d "$root/target/qualification/setup-evidence/attempt.XXXXXX")
phase=setup
"$launcher" setup "$directory" "$evidence/launcher" "$(command -v curl)" "$(command -v gzip)" > "$evidence/setup.stdout" 2> "$evidence/setup.stderr" || status=$?
files=("$evidence/setup.stderr")
if (( status == 0 )); then
    phase=check
    "$launcher" check "$directory" > "$evidence/check.stdout" 2> "$evidence/check.stderr" || status=$?
    files=("$evidence/check.stderr")
else
    for file in download.stderr prepare.txt server.stderr; do
        if [[ -f "$evidence/launcher/$file" ]]; then files+=("$evidence/launcher/$file"); fi
    done
fi
if (( status != 0 )); then
    printf 'ic-testkit-server %s failed (exit %d); evidence: %q\n' "$phase" "$status" "$evidence" >&2
    perl scripts/collect-setup-evidence.pl "${STORE_SETUP_EVIDENCE:---console-only}" "${files[@]}" >&2 || \
        echo 'Could not collect simulator diagnostics; original setup failure is retained.' >&2
else
    cat "$evidence/check.stdout"
fi
exit "$status"
