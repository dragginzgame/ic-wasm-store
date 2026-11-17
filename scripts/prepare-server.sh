#!/usr/bin/env bash
# The frozen launcher owns provisioning; the current Testkit library owns no server.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
status=0
launcher=$(bash scripts/testkit-launcher.sh check)
directory=${IC_TESTKIT_SERVER_DIRECTORY:-$root/target/qualification/server}
[[ "$directory" == /* ]] || directory="$root/$directory"
directory=${directory%/}
mkdir -p target/qualification/setup-evidence
evidence=$(mktemp -d "$root/target/qualification/setup-evidence/attempt.XXXXXX")
phase=setup
"$launcher" setup --directory "$directory" > "$evidence/setup.stdout" 2> "$evidence/setup.stderr" || status=$?
files=("$evidence/setup.stderr")
if (( status == 0 )); then
    phase=check
    "$launcher" check --directory "$directory" > "$evidence/check.stdout" 2> "$evidence/check.stderr" || status=$?
    files=("$evidence/check.stderr")
else
    # Report only the attempt named by this invocation, never an older failure.
    prefix="PocketIC setup evidence: $directory/.setup-v1-"
    while IFS= read -r line; do
        [[ "$line" == "$prefix"* ]] || continue
        suffix=${line#"$prefix"}
        [[ "$suffix" =~ ^[0-9]+-[0-9]+$ ]] || continue
        files+=("$directory/.setup-v1-$suffix/download.stderr" "$directory/.setup-v1-$suffix/failure.txt")
        break
    done < "$evidence/setup.stderr"
fi
if (( status != 0 )); then
    printf 'ic-testkit-server %s failed (exit %d); evidence: %q\n' "$phase" "$status" "$evidence" >&2
    if [[ -n "${STORE_SETUP_EVIDENCE:-}" ]]; then
        perl scripts/collect-setup-evidence.pl "$STORE_SETUP_EVIDENCE" "${files[@]}" || \
            echo 'Could not collect simulator diagnostics; original setup failure is retained.' >&2
    fi
    bash scripts/show-server-setup-failure.sh "${files[@]}" >&2 || echo 'Could not render setup evidence; original setup failure is retained.' >&2
else
    cat "$evidence/check.stdout"
fi
exit "$status"
