# IC Wasm Store

A standalone, controller-authorized Internet Computer canister for immutable,
versioned Wasm custody. It retains resumable uploads, shared chunks, quotas and
operation pins, and prepares the management chunk cache without installing code.

Canic is its first consumer. This repository has no Canic runtime, Host or CLI
dependency. Deployment approvals, controller management and fleet orchestration
belong to the consuming application.

## Behavior and limits

A controller prepares an exact release manifest, uploads missing chunks, and
publishes after Store verifies chunk geometry, hashes, the raw Wasm header and
the complete artifact digest. Publication does **not** validate the full Wasm
module or approve installation. Release labels cannot be reassigned to different
content; labels sharing an artifact reuse its stored bytes.

Artifacts are limited to 8 MiB, transferred in 256 KiB chunks. Default quotas are
128 MiB of reserved artifact payload and 1,024 releases; configurable ceilings
are 256 MiB and 4,096 releases. Reservations include incomplete artifacts and
exclude metadata. All endpoints, including reads, require a controller.

Pins retain exact caller-owned operations, with at most 32 pins per artifact.
Controllers can `delete` a published release or `cancel` an unpublished upload
by supplying its exact manifest. Both refuse artifacts with retained pins.
Removal leaves a permanent tombstone: `prepare` can restore the same manifest,
but cannot assign different bytes to that name/version. Payload reservations
and unshared chunks are reclaimed when the last active alias is removed.
Tombstones still occupy release-label quota; there is no unpin operation.
The replicated `tombstones` endpoint lists removed manifests in bounded pages.
See [removal semantics](docs/design/extraction.md#removal-and-restoration) for
replay, storage and upgrade constraints. Cache readiness is an
observation, not a guarantee that the management cache will remain available.
Closing maintenance admission blocks new mutation work but does not drain
outstanding calls; the consuming application owns quiescence and upgrades.

This remains experimental. Availability of older code does not imply safe
application-state rollback.

## Development

Use Rust 1.100.0. From the repository root, prepare tools and the Wasm target:

```sh
rustup target add --toolchain 1.100.0 wasm32-unknown-unknown
make install-tools
make tools-check
```

The main checks are:

| Command | Scope |
| --- | --- |
| `make test` | Product workspace protocol and persistence tests |
| `make clippy` | Product native/Wasm and qualification lint checks |
| `make doc` | Both workspaces' documentation, with warnings denied |
| `make build` | Standalone Store Wasm, without qualification features |
| `make qualification-setup` | Explicit PocketIC server preparation |
| `make qualification-test` | Builds both test executables and runs installed qualification |
| `make ci` | Aggregate formatting, tooling admission, tests, lints, docs and installed qualification |

For focused work, use `make test-protocol` or `make test-store`, and select
`make clippy-native`, `make clippy-wasm` or `make clippy-qualification` as appropriate.
`make check` checks only native product targets; the canister entrypoints are
Wasm-only and are checked by `make clippy-wasm`. `make help` lists these targets.

Run `make qualification-setup` before the installed test or `make ci` on a fresh
checkout. Setup may download the server; the test checks the prepared bundle.
The `maintenance-candidate` feature identifies a qualification executable and
must not be selected for live deployment. See [qualification](qualification/README.md)
for coverage and limitations. Linux and macOS are intended development hosts;
the recorded execution evidence is Linux only.

The product workspace contains `ic-wasm-store` and the passive
`ic-wasm-store-protocol`. The independent `qualification/` workspace owns the
installed tests and its own lockfile. Both lockfiles are committed, and all
packages are unpublished. See the [extraction contract](docs/design/extraction.md)
for provenance and retained compatibility obligations.
