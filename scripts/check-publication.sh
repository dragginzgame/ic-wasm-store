#!/usr/bin/env bash
# Consumer admission; shared tooling owns packaging provenance and publication.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
fail() { printf 'Store publication refused: %s\n' "$*" >&2; exit 1; }
dirty=$(git status --porcelain --untracked-files=all)
[[ -z "$dirty" ]] || fail 'commit the intended source before publication'
source=$(git rev-parse HEAD)
repository=dragginzgame/ic-wasm-store
# Require the complete hosted workflow for this exact pushed commit, including
# both supported hosts. A successful pull-request merge commit is not equivalent.
runs=$(gh api --method GET "repos/$repository/actions/workflows/ci.yml/runs" \
    -f head_sha="$source" -f event=push -f per_page=100)
jq -e --arg source "$source" '[.workflow_runs[] | select(.head_sha == $source and .event == "push" and .head_branch == "main")] | sort_by(.run_number, .run_attempt) | last | .status == "completed" and .conclusion == "success"' <<< "$runs" > /dev/null \
    || fail 'the latest main push CI run for HEAD must have succeeded'
metadata=$(cargo metadata --locked --offline --no-deps --format-version 1)
arguments=()
for package in ic-wasm-store-protocol ic-wasm-store; do
    version=$(jq -er --arg name "$package" '.packages[] | select(.name == $name) | .version' <<< "$metadata")
    arguments+=(--package "$package@$version" "crates/$package")
done
# Keep the archive producer and provenance reader on the same explicit path,
# even when the caller configures CARGO_TARGET_DIR or build.target-dir.
cargo package --locked --target-dir "$root/target" --target wasm32-unknown-unknown -p ic-wasm-store-protocol -p ic-wasm-store
mkdir -p target/publication
attempt=$(mktemp -d "$root/target/publication/admission.XXXXXX")
bash vendor/toko-tooling/scripts/ci/check-cargo-package-provenance.sh \
    "$attempt/provenance" "$root/target/package" "https://github.com/$repository.git" "$source" "${arguments[@]}"
observed=$(git rev-parse HEAD)
dirty=$(git status --porcelain --untracked-files=all)
[[ "$observed" == "$source" && -z "$dirty" ]] || fail 'source changed during admission'
printf 'Hosted CI and archive provenance admitted for %s\n' "$source"
