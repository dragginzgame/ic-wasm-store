# IC Wasm Store

Store 0.10 requires fresh backing. Keep an existing 0.9 Store intact while preparing
a separate Store, republishing trusted artifacts and reconciling callers, pins and
maintenance obligations before cutover. In-place upgrades from 0.9 refuse the old
image. Reinstalling on the same canister erases its state.

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

Artifacts are limited to 8 MiB, transferred in 1 MiB chunks.
To republish an earlier artifact, rebuild its manifest from the original Wasm. Default quotas are
128 MiB of reserved artifact payload and 1,024 releases; configurable ceilings
are 256 MiB and 4,096 releases. Reservations include incomplete artifacts and
exclude metadata. A controller can grant a named installer access to one pinned
release through `read_granted`. The other endpoints require a controller.
Store inspects external ingress before induction. It accepts known methods within
their declared request budgets from controllers. For `read_granted`, it also
admits a named installer with an unexpired grant from a current controller.
Inspection reads the method, caller and size without copying the payload.
The handler checks the complete grant, caller, deadline and issuer again.
Queries and inter-canister calls go straight to the handler.

Pins retain exact caller-owned operations, with at most 32 pins per artifact.
Removing a controller leaves its pins in place. They still prevent removal of
every alias and retain the artifact’s payload reservation. Account for those
obligations before rotating controllers.
Controllers call `remove` with `Delete` for a published release or `Cancel` for an unpublished upload
by supplying its exact manifest. Both refuse artifacts with retained pins.
Removal retains the identity as a tombstone: `prepare` replaces it with an active
release only for the same manifest, and cannot assign different bytes to that
name/version. Restoration rechecks payload quota; when no active alias retains
the published artifact, missing bytes must be uploaded and publication repeated.
Removing the last active alias reclaims its payload reservation and unshared chunks.
Tombstones still occupy release-label quota; there is no unpin operation.
The replicated `observe(Tombstones)` operation lists removed manifests in bounded pages.
Queries `inspect(Status)` and `inspect(List)` expose active releases; `Info.releases` counts active
labels plus tombstones, and `Info.artifacts` includes retained tombstone metadata.
See [removal semantics](docs/design/extraction.md#removal-and-restoration) for
replay, storage and upgrade constraints. Cache readiness is an
observation, not a guarantee that the management cache will remain available.
Closing maintenance admission blocks new mutation work but does not drain
outstanding calls; the consuming application owns quiescence and upgrades.
To recover a closed attempt after its owner loses controller status, a current
controller reads `admission(Observe)`, reconciles outstanding maintenance with
the operator, then submits `Activate` with that exact sequence and operation.
The original owner and sequence remain recorded, and closed-era grants stay invalid.

`inspect` and `read` queries return uncertified data. For integrity-sensitive
reads, obtain the exact published manifest via replicated `observe(Status)`,
check each chunk against its hash and verify the assembled length and digest.
Use a replicated observation for decisions that depend on current Store state.

`prepare_cache` adds missing chunks to the management chunk store. Distinct
artifacts can fill it even after Store releases are removed. A controller can
call `clear_chunk_store` on the management canister for this Store. It clears
all cached chunks, so coordinate with pending installations first and prepare
the pinned artifacts again afterward. Stable Store payloads survive cache clearing.

With the replacement Platform Runtime, stop the canister and wait for `Stopped`
before upgrading, then restart it. External ingress refused by inspection receives IC reject code 4. A guard refusal
during handler execution traps with code 5.
Init and post-upgrade arguments are limited to 16 KiB. Version 0.9 groups metadata under `inspect` (query) and `observe` (update).
Store 0.10 keeps those Wire methods and introduces the storage cut described above.

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

`make install-tools` also prepares the frozen Testkit 0.110.5 launcher through
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
| `make qualification-test` | Builds both qualification Wasms and runs installed tests. Set `QUALIFICATION_TEST` to a test name for focused execution |
| `make qualification-predecessor-test` | Builds published 0.9.0 and verifies refusal and rollback when upgrading its old image |
| `make release-verify` | Formatting, tooling admission, product tests, lints, docs and both installed qualification suites |
| `make check-wire-api` | Compare mounted Wire contracts and release Wasm exports with the reviewed baseline |
| `make test-tooling` | Publication, setup, diagnostics, path and delivery caller fixtures |
| `make ci` | Release verification followed by the Wire baseline and tooling caller checks |

For focused work, use `make test-protocol` or `make test-store`, and select
`make clippy-native`, `make clippy-wasm` or `make clippy-qualification` as appropriate.
`make check` checks only native product targets; the canister entrypoints are
Wasm-only and are checked by `make clippy-wasm`. `make help` lists these targets.

Run `make qualification-setup` after tool installation and before the installed
test or `make ci` on a fresh checkout. Setup admits the prepared 0.110.5 launcher
and may download the server. Tests use the Testkit client from the qualification
lockfile with that prepared bundle.
The launcher owns one server for the test command. Alternatively, supply
`IC_TESTKIT_POCKET_IC_URL` for an externally owned server; see the
[qualification ownership contract](qualification/README.md).
CI retains bounded, sanitized scanner, compiler and fixture setup diagnostics
for seven days on failure. `make test-setup-evidence` checks the collection and
launcher-selection failure paths without network access.

### Wire API baseline

PR CI regenerates [the Wire manifests](api/wire) from the mounted requests,
replies, method IDs and limits. It verifies the name and call mode of every
method against the same release Wasm used for installed qualification. The other three canister exports are
initialization, post-upgrade and ingress inspection. Each filename records the method's
call mode.

To check a standalone build from the repository root:

```sh
make build
make check-wire-api WIRE_API_WASM=target/wasm32-unknown-unknown/release/ic_wasm_store.wasm
```

For an intentional API change, review the diff and candidate directory reported
by the check. Include the corresponding additions, changes and removals in
`api/wire` for PR review. Check representative supported client bytes if the
encoding changes.

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
requires explicit authorization, a clean committed source, a passing `make release-verify`,
and archive provenance verification against the pushed source commit. Both Make
publication targets run `make release-verify` before archive admission. This runs
Clippy first, then formatting/tooling admission, product tests and docs, then the
complete installed suite and published-predecessor rollback. Both installed suites
share the current original/candidate Wasm builds within that invocation.

Tooling caller regressions run through `make test-tooling` when affected and in
repository CI, rather than every publication. Archive creation, pushed-source
provenance and registry admission remain fresh for every upload. The coordinated
product pair retains its full product gate even when publishing one package;
protocol changes affect Store and its installed witnesses.

For an authorized delivery, let `make publish` own qualification; do not precede
it with `make ci` or `make publish-check` solely as another release gate.
Independent invocations freshly build the installed artifacts and may reuse
their two execution proofs on Linux x86_64 through Toko Tooling's exact-input
runner. Changed artifacts, source, runtime libraries or bounded environment
invalidate reuse; missing or damaged proofs execute again. Other hosts and
custom runtime selections execute normally. Lint, native tests, documentation,
source/archive and registry admission remain fresh. See the
[delivery and recovery contract](docs/operations/delivery.md) for the stage roster,
input scope, limitations and partial-upload recovery.

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
