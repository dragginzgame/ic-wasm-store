# Extraction from Canic

Source: [dragginzgame/canic at c39aee1](https://github.com/dragginzgame/canic/tree/c39aee1e34dae82be6da89bf6242c3a0e22ade84) (v0.214.0), `crates/canic-wasm-store` and `crates/canic-wasm-store-protocol`. The source remains preserved in that repository's Git history.

The packages are now ic-wasm-store and ic-wasm-store-protocol. Rust import names change; the initial extraction otherwise preserves runtime and protocol source. Endpoint method names/numbers, Wire shapes, record magic, Storage authority/key names, quotas and maintenance sequence semantics are unchanged. New compilation produces a new executable identity: deployed upgrades still need explicit approval and qualification, never an implicit replacement.

Store owns release/chunk identity, verification, quota reservations, retained pins, cache reconstruction, admission closure and preservation observations. Controllers supply exact operation commitments; controller membership grants no authority in a consuming application's user model. Root owns deployment approvals, installation management effects and recovery in Canic; Store performs its own management-cache observation and uploads. At the source revision, Root supported one retained Store-maintenance operation. Extraction did not implement repeatable Root maintenance; this is not a claim about later Canic revisions.

Reused legacy research is recorded in Canic's docs/qualification/history.md at the source revision, section Versioned Wasm Store decision, inspecting canic-legacy 377fbceb3994c9d1f3033979ef4e4ce9e6b85801. Keep immutable versions, authoritative stable bytes and reconstructible management cache; do not import historical mutable-version replacement or the legacy control plane.

Supplier tests own persistence, metadata budgets and standalone installed behavior. Canic retains Root-mediated publication, interrupted management effects, preservation custody and complete CLI fleet journeys. Native and installed tests move with their owning behavior; no Canic dependency is introduced merely to run supplier checks.

The consumer integration contract is to select an immutable Git revision of the protocol and build the deployable Store from that same revision and its committed lockfile. No sibling checkout, path override, copied implementation or registry publication is required.

## Initial extraction evidence (2026-11-15)

Linux: 10 Store state tests and one protocol budget test pass, along with native/Wasm Clippy and both workspaces' Rustdoc, formatting and tooling admission. The standalone installed witness passes with the extracted original/candidate pair and with Canic's retained pre-extraction executable (SHA-256 `5bf18e1ee35c8c84ebaba44d713cd87bc8759dfe6bb033f707307748ca21df25`) upgraded to the extracted candidate. Both preserve and resume an 8 MiB upload, retain two versions and reconstruct exact bytes. The original run wrote logs under `target/qualification/extraction-*`; that directory is ignored build output, not committed or durable evidence. These results record the initial extraction and do not qualify subsequent changes. This does not qualify mainnet deployment, a new maintenance sequence, or native macOS execution.

## Removal and restoration

`remove` (Wire method 122) accepts `Delete(Manifest)` for a published release or
`Cancel(Manifest)` for an unpublished upload. Both return the removed manifest. A phase
or identity mismatch, or any retained pin on the shared artifact, returns
`Conflict`. Missing labels return `NotFound`; closed admission returns `Closed`.
An exact replay against a tombstone succeeds for either operation. All these
endpoints are controller-only. No pin is removed implicitly.

Removal affects one label. Active aliases retain the artifact and its payload
reservation. Removing the last active alias releases its reservation and removes
only chunks not referenced by another active artifact, including incomplete
uploads. This scans bounded metadata, not payload bytes. Storage can reuse freed
collection space; the operation does not shrink IC stable memory or remove
management-cache copies. Installed qualification covers cancellation at 4,096
labels with 31 maximum-size reservations, including an 8 MiB cancellation and
retention of shared pinned bytes. This proves simulator completion for the tested
catalog shape, not a production instruction-cost bound; see the
[qualification boundaries](../../qualification/README.md#qualification-boundaries).

The full digest and chunk geometry remain retained. `prepare` restores only the
same manifest, re-admitting payload quota when necessary; `put` and `publish`
do not restore a tombstone. Restored content must be uploaded and verified again
unless another active alias already retains it. Labels are never reusable for
different content. Active labels plus tombstones count toward release quota;
`Info.artifacts` includes retained metadata for removed artifacts.

`observe(Tombstones)` (121) takes `Option<Release>` as an exclusive pagination cursor and
returns up to `PAGE_SIZE` manifests. `inspect(Status)` and `inspect(List)` expose active
releases only. Preservation enumeration distinguishes active and removed entries under the exact closed admission attempt.
Use each returned release as the next cursor; do not infer an independent sort
order. Closed admission prevents mutation throughout this enumeration.

Requests carry content identity, not a restoration-generation token. Controllers
must settle outstanding delete/cancel requests before restoring the same label:
a delayed matching removal can affect that restored incarnation. Restoration
is explicit through `prepare`; this API does not provide an operation journal.

In the original 0.2 removal release, the four checked allocation names and existing active record encodings remained
unchanged. Removed release keys use byte `0x01` instead of `0x00` between name
and version, within the original key size bound; the artifact record retains
its original geometry. Existing stores reopen without resetting collections.
Older executables reject tombstone keys, so rollback after removal requires an
explicit compatible executable and qualification. This feature changes consumer
semantics and belongs to the 0.2 minor line; registry publication does not imply deployment.

## Explicit preservation entries

`observe_preservation` (117) accepts the existing closed-attempt Request and
returns Page. Entry::Active retains Custody; Entry::Removed retains the exact
Manifest. Entry::order_key defines name, active-before-removed, then version;
use the returned manifest's release as the exclusive cursor. The method performs
one ordered-map range step per page, without a catalog copy or payload scan.
The exact closed attempt and captured controller are checked on every page.
Consumers hash the complete discriminated entry, include removed labels in quota
and counts, and require an explicit end page. They must not compare only release
name/version or infer removal from empty upload progress.

Store 0.8 exposes this single preservation endpoint. Complete old maintenance
attempts using their original Root and Store before switching. Outstanding
removals must still settle before restoring an identical label.

## Platform replacement adoption (2026-11-16)

Store requires published Runtime 0.23.14, Protocol 0.3.2 and Types 0.21.2.
Protocol's management-chunks capability supplies the bounded stored_chunks and
upload_chunk codecs requested in
[Platform #16 — Bounded management chunk-cache codecs](https://github.com/dragginzgame/ic-platform/issues/16).
The old Platform facade is absent from the product graph. Store owns its Wire
frames, controller admission, pins, hash comparison, deadlines and callback fences;
Platform owns the external management formats and execution boundary.

The reply decoder preserves the 65,536-byte, 4,096-header-byte, 256-type,
131,072-work and 262,144-allocation budgets. New vector and skipped-value budgets
are explicit. The canister uses the new Runtime's single executor and explicit
entrypoint guards, fallible codecs and non-returning error handler. Guard refusals
now trap (IC reject code 5 instead of the old explicit rejection code 4), and
lifecycle arguments are bounded to 16 KiB. These observable changes select the
pending 0.3 minor line. Method names/numbers, encoded records, allocation names
and retained maintenance fences remain unchanged.

Stop and wait for IC status Stopped before upgrade, then restart: suspended
Runtime callbacks are heap-local and must drain first. The generated export
attributes use the supplier's narrowly scoped unsafe-lint expectations under
workspace deny; state and passive protocol modules retain unsafe forbid.

Storage 0.74.1 still owns the Runtime 0.22.5 ic0-only stable-memory adapter. The
selected Wasm feature graph has no old executor, macros or runtime guard. This is
an explicit retained supplier edge, not complete elimination of the old Runtime.
The new Runtime supplies the stable-memory primitives requested in
[Platform #14 — Stable-memory boundary](https://github.com/dragginzgame/ic-platform/issues/14),
but Storage has not published its consumer transition.
[Storage #3 — Wasm Store retained-state migration](https://github.com/dragginzgame/ic-storage/issues/3)
continues to own that remaining work. Store neither bypasses Storage nor silently
moves existing images to the fresh persistence rewrite.

The installed qualification covers management-cache preparation/reuse, controller
revocation and close/reopen callback fences, removal at capacity, and retained
state across upgrade. See the qualification workspace for the actual scenarios;
local evidence does not establish mainnet deployment or native macOS acceptance.
## Pinned delivery

A controller can authorize replicated delivery of one exact retained pin to a
named installer for up to five minutes. The installer supplies the unchanged
grant and the issuing controller's principal. Store checks the actual caller,
current controller authority, open maintenance sequence, retained pin and
deadline before returning a verified chunk. Issuing or using a grant has no
effect on publication, pins or installation authority.

The settings allocation stores at most 32 grant fingerprints. In Store 0.9, settings without
grants retained the original `CWS3` encoding. Settings with grants used `CWS4`,
which earlier executables refuse. A maintenance closure clears the grants before
preservation starts and fences any reissued request from the prior sequence.

## Store 0.10 storage cut

Each label references a SHA-256 identity over the domain `Store artifact geometry v1`,
the claimed 32-byte content digest, the eight-byte big-endian length and the ordered
32-byte chunk hashes. Exact aliases share quota, progress and pins. A distinct
layout gets independent metadata and reservation, even if it claims the same
content digest. Publication verifies the payload against the content digest.
Tombstones retain their exact layout for replay, restoration and preservation.

The four allocation names are retained. Artifact records use `CWA4`, settings
use `CWS6` with the issuer and installer alongside each retained grant fingerprint, and a 0.9 image is refused on reopen. Deploy 0.10 on fresh backing.
A surviving controller can activate an exact orphaned closure after the captured
owner loses controller status. Preservation stays scoped to the captured owner
while closed. Activation preserves its owner and sequence and never restores grants.
