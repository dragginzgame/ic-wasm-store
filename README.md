# IC Wasm Store

A standalone, controller-authorized Internet Computer canister for immutable,
versioned Wasm custody. It retains resumable uploads, shared chunks, quotas and
operation pins, and prepares the management chunk cache without installing code.

Canic is its first consumer. This repository has no Canic runtime, Host or CLI
dependency. Deployment approvals, controller management and fleet orchestration
belong to the consuming application.

## Packages

| Package | Intended use |
| --- | --- |
| [ic-wasm-store](https://crates.io/crates/ic-wasm-store) | Standalone canister implementation; build for `wasm32-unknown-unknown` |
| [ic-wasm-store-protocol](https://crates.io/crates/ic-wasm-store-protocol) | Passive request/response types, method identifiers and frame limits for clients and canisters |

The canister crate exposes no native service API. The protocol crate supplies
message types, not a transport client. See the [protocol API](https://docs.rs/ic-wasm-store-protocol)
for constructing requests.

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
Removal retains the identity as a tombstone: `prepare` replaces it with an active
release only for the same manifest, and cannot assign different bytes to that
name/version. Restoration rechecks payload quota; when no active alias retains
the published artifact, missing bytes must be uploaded and publication repeated.
Payload reservations
and unshared chunks are reclaimed when the last active alias is removed.
Tombstones still occupy release-label quota; there is no unpin operation.
The replicated `tombstones` endpoint lists removed manifests in bounded pages.
Ordinary `status` and `list` expose active releases; `Info.releases` counts active
labels plus tombstones, and `Info.artifacts` includes retained tombstone metadata.
See [removal semantics](docs/design/extraction.md#removal-and-restoration) for
replay, storage and upgrade constraints. Cache readiness is an
observation, not a guarantee that the management cache will remain available.
Closing maintenance admission blocks new mutation work but does not drain
outstanding calls; the consuming application owns quiescence and upgrades.

With the replacement Platform Runtime, stop the canister and wait for `Stopped`
before upgrading, then restart it. Controller and oversized-frame guard failures
trap (IC reject code 5); clients must not rely on the former rejection code 4.
Init and post-upgrade arguments are limited to 16 KiB. Store endpoint names,
Wire encodings and retained allocation/record identities are unchanged.

This remains experimental. Availability of older code does not imply safe
application-state rollback.

## Development

Install Git, rustup and GNU Make first. From the repository root, install the
pinned compiler, its components and the Wasm target, then prepare repository tools:

```sh
rustup toolchain install 1.100.0 --profile minimal --component rustfmt,clippy --target wasm32-unknown-unknown
make install-tools
make tools-check
make install-hooks
```

`make install-tools` also prepares the frozen Testkit 0.106.0 launcher through
the shared Cargo installer. `make tools-check` admits that selection offline.
Existing checkouts using the former `.tools/testkit-bootstrap` copy must rerun
`make install-tools`; qualification no longer selects that copy automatically.

Run `make install-hooks` once in each fresh checkout. It configures this
repository's `core.hooksPath` to the shared formatting hook; Git configuration
is local and is not carried by a clone. The hook formats staged files using the
same product and qualification workspace coverage as `make fmt`/`make fmt-check`.
It refuses partially staged files and never installs tools during a commit.

The main checks are:

| Command | Scope |
| --- | --- |
| `make test` | Product workspace protocol and persistence tests |
| `make clippy` | Product native/Wasm and qualification lint checks |
| `make doc` | Both workspaces' documentation, with warnings denied |
| `make build` | Standalone Store Wasm, without qualification features |
| `make qualification-setup` | Explicit PocketIC server and released-archive preparation |
| `make qualification-test` | Builds both test executables and runs installed qualification |
| `make qualification-predecessor-test` | Builds published 0.2.2 and tests its upgrade to the current candidate |
| `make ci` | Aggregate formatting, tooling admission, tests, lints, docs and installed qualification |

For focused work, use `make test-protocol` or `make test-store`, and select
`make clippy-native`, `make clippy-wasm` or `make clippy-qualification` as appropriate.
`make check` checks only native product targets; the canister entrypoints are
Wasm-only and are checked by `make clippy-wasm`. `make help` lists these targets.

Run `make qualification-setup` after tool installation and before the installed
test or `make ci` on a fresh checkout. Setup admits the prepared 0.106.0 launcher
and may download the server; tests use Testkit 0.107.3 clients and the prepared bundle.
The launcher owns one server for the test command. Alternatively, supply
`IC_TESTKIT_POCKET_IC_URL` for an externally owned server; see the
[qualification ownership contract](qualification/README.md).
CI retains bounded, sanitized scanner, compiler and fixture setup diagnostics
for seven days on failure. `make test-setup-evidence` checks the collection and
launcher-selection failure paths without network access.

The `maintenance-candidate` feature identifies a qualification executable and
must not be selected for live deployment. See [qualification](qualification/README.md)
for coverage and limitations. Linux and macOS are intended development hosts;
the recorded execution evidence is Linux only.

The product workspace contains `ic-wasm-store` and the passive
`ic-wasm-store-protocol`. The independent `qualification/` workspace owns the
installed tests and its own lockfile. Both lockfiles are committed, and all
product packages are published to crates.io; qualification remains unpublished.
See the [extraction contract](docs/design/extraction.md)
for provenance and retained compatibility obligations.

## Publication

Both product crates share a coordinated version. `make publish-check` verifies their
package archives without uploading; `make publish` publishes the declared product
roster in dependency order. Qualification packages are excluded. Publication
requires explicit authorization, a clean committed source, a passing `make ci`,
and archive provenance verification against the pushed source commit. Both Make
publication targets run the existing local `make ci` gate before archive admission.
Hosted Linux/macOS CI and the single history secret scan run asynchronously;
pending, queued or absent hosted results alone do not block a routine patch.
Publication admission packages and verifies both crates for
`wasm32-unknown-unknown`, checks their source identity with the shared provenance
helper, and retrieves that commit from the declared GitHub repository. Cargo's
native package verification follows in the shared publication adapter.
Known relevant failures still require resolution or explicit disposition. Enforced
protections and maintainer-selected qualification for a named release or changed
contract remain binding; report their source before waiting. Publication and CI
status are reported separately. Publishing a crate does not deploy a canister.
