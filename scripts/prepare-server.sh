#!/usr/bin/env bash
# The frozen launcher owns provisioning; the current Testkit library owns no server.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
status=0
launcher=${TESTKIT_RELEASE_LAUNCHER:-$root/.tools/testkit-bootstrap/bin/ic-testkit-server}
directory=${IC_TESTKIT_SERVER_DIRECTORY:-$root/target/qualification/server}
[[ "$directory" == /* ]] || directory="$root/$directory"
directory=${directory%/}
[[ "$("$launcher" --version)" == 'ic-testkit-server 0.106.0' ]] || {
    echo 'Prepare the reviewed ic-testkit-server 0.106.0 bootstrap with make qualification-setup.' >&2
    exit 2
}
mkdir -p target/qualification/setup-evidence
evidence=$(mktemp -d "$root/target/qualification/setup-evidence/attempt.XXXXXX")
"$launcher" setup --directory "$directory" > "$evidence/setup.stdout" 2> "$evidence/setup.stderr" || status=$?
files=("$evidence/setup.stderr")
if (( status == 0 )); then
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
    bash scripts/show-server-setup-failure.sh "${files[@]}" >&2 || echo 'Could not render setup evidence; original setup failure is retained.' >&2
else
    cat "$evidence/check.stdout"
fi
exit "$status"
