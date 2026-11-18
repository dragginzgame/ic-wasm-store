#!/usr/bin/env bash
# Consumer admission; shared tooling owns packaging provenance and publication.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
fail() { printf 'Store publication refused: %s\n' "$*" >&2; exit 1; }
dirty=$(git status --porcelain --untracked-files=all)
[[ -z "$dirty" ]] || fail 'commit the intended source before publication'
source=$(git rev-parse HEAD)
repository=dragginzgame/ic-wasm-store
# Local validation is selected by the Make publication adapter. Hosted CI is
# reported separately; pending results do not gate archive admission.
metadata=$(cargo metadata --locked --no-deps --format-version 1)
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
printf 'Source and archive provenance admitted for %s\n' "$source"
