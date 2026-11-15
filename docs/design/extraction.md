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

`delete` (Wire method 114) accepts an exact published `Manifest`; `cancel` (115)
accepts an exact unpublished manifest. Both return the removed manifest. A phase
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

`tombstones` (116) takes `Option<Release>` as an exclusive pagination cursor and
returns up to `PAGE_SIZE` manifests. `status` and ordinary `list` expose active
releases only. Preservation enumeration includes tombstones as unpublished
entries with all chunks missing and no pins; enumerate `tombstones` under the
same closed admission attempt to distinguish removed identities from uploads.
Use each returned release as the next cursor; do not infer an independent sort
order. Closed admission prevents mutation throughout this enumeration.

Requests carry content identity, not a restoration-generation token. Controllers
must settle outstanding delete/cancel requests before restoring the same label:
a delayed matching removal can affect that restored incarnation. Restoration
is explicit through `prepare`; this API does not provide an operation journal.

The four checked allocation names and existing active record encodings remain
unchanged. Removed release keys use byte `0x01` instead of `0x00` between name
and version, within the original key size bound; the artifact record retains
its original geometry. Existing stores reopen without resetting collections.
Older executables reject tombstone keys, so rollback after removal requires an
explicit compatible executable and qualification. This feature changes consumer
semantics and belongs to the 0.2 minor line; registry publication does not imply deployment.
