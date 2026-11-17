#!/usr/bin/env bash
# Frozen published input; never substitute the current checkout for this fixture.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
version=0.2.2
checksum=5ac8eeefd0b49bc36a1e946bc984dcabae974cb6510150296d90ae205d5e1cd0
archive="$root/target/qualification/ic-wasm-store-$version.crate"
mkdir -p target/qualification
case "${1:-}" in
    setup)
        if [[ ! -f "$archive" ]]; then
            temporary=$(mktemp "$archive.XXXXXX")
            trap 'rm -f "$temporary"' EXIT
            curl --fail --location --retry 2 --connect-timeout 15 --max-time 120 \
                "https://static.crates.io/crates/ic-wasm-store/ic-wasm-store-$version.crate" -o "$temporary"
            bash vendor/toko-tooling/scripts/ci/verify-file-checksum.sh sha256 "$checksum" "$temporary"
            mv "$temporary" "$archive"
        fi
        ;;
    build) ;;
    *) echo 'usage: prepare-predecessor.sh setup|build' >&2; exit 2 ;;
esac
[[ -f "$archive" ]] || { echo 'Run make qualification-setup to prepare the released predecessor' >&2; exit 1; }
bash vendor/toko-tooling/scripts/ci/verify-file-checksum.sh sha256 "$checksum" "$archive"
[[ "$1" == build ]] || exit 0
source_dir=$(mktemp -d "$root/target/qualification/predecessor-source.XXXXXX")
trap 'rm -rf "$source_dir"' EXIT
tar -xzf "$archive" -C "$source_dir"
cargo build --locked --release --target wasm32-unknown-unknown \
    --manifest-path "$source_dir/ic-wasm-store-$version/Cargo.toml" \
    --target-dir "$root/target/qualification/predecessor-build" \
    --message-format=json-render-diagnostics > target/qualification/predecessor.jsonl
wasm=$(jq -sr '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_wasm_store") | .filenames[] | select(endswith(".wasm"))] | unique | if length == 1 then .[0] else error("expected released Store Wasm") end' target/qualification/predecessor.jsonl)
cp "$wasm" target/qualification/predecessor.wasm
