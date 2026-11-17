#!/usr/bin/env bash
# Select the frozen launcher through the shared Cargo installer and offline admission.
set -euo pipefail
root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
case "${1:-}" in
    setup) options=() ;;
    check) options=(--check) ;;
    *) echo 'usage: testkit-launcher.sh setup|check' >&2; exit 2 ;;
esac
launcher=${TESTKIT_RELEASE_LAUNCHER:-}
if [[ -z "$launcher" ]]; then
    launcher=$(bash "$root/vendor/toko-tooling/scripts/dev/install-rust-tools.sh" \
        --consumer "$root" --package ic-testkit --version 0.106.0 \
        --bin ic-testkit-server --profile release --features cli "${options[@]+"${options[@]}"}")
fi
[[ "$("$launcher" --version)" == 'ic-testkit-server 0.106.0' ]] || {
    echo 'Prepare the reviewed ic-testkit-server 0.106.0 launcher with make install-tools.' >&2
    exit 2
}
printf '%s\n' "$launcher"
