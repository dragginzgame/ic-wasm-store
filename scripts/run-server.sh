#!/usr/bin/env bash
# Give each real execution its own Testkit diagnostics directory.
set -euo pipefail
umask 077
launcher=$1
directory=$2
shift 2
attempt=$(mktemp -d "$PWD/target/qualification/server-run.XXXXXX")
exec "$launcher" run "$directory" "$attempt/evidence" --ttl 1800 -- "$@"
