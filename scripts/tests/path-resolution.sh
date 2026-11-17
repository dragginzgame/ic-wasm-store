#!/usr/bin/env bash
# Root resolution must ignore both CDPATH output and alternate search roots.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
repo="$fixture/repo with spaces"
mkdir -p "$repo/scripts" "$fixture/bin" "$fixture/search/scripts"
cp "$root/scripts/test-installed.sh" "$root/scripts/prepare-predecessor.sh" "$repo/scripts/"
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
[[ "$PWD" == "$EXPECTED_ROOT" ]] || exit 98
printf 'reached Cargo in expected repository\n'
exit 43
STUB
chmod +x "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH" EXPECTED_ROOT="$repo"
for search in . "$fixture/search:."; do
    for script in test-installed prepare-predecessor; do
        status=0
        (cd "$repo" && CDPATH="$search" bash "scripts/$script.sh" invalid) > "$fixture/output" 2>&1 || status=$?
        if [[ "$script" == test-installed ]]; then
            [[ "$status" == 43 ]]
            rg -q '^reached Cargo in expected repository$' "$repo/target/qualification/original.jsonl"
        else
            [[ "$status" == 2 ]]
            rg -q '^usage: prepare-predecessor.sh setup\|build$' "$fixture/output"
        fi
    done
done
printf 'path resolution: relative invocation, spaces, CDPATH output/search and exit statuses passed\n'
