# Changelog

## [0.2.3] - 2026-11-16

### ➕ Added

- Add `observe_preservation_v2` (Wire 117): closed-attempt pages explicitly
  distinguish active uploads/releases from removed labels and expose the exact
  comparable order. Historical method 113 and stable encodings are unchanged.
  Fixes [#2](https://github.com/dragginzgame/ic-wasm-store/issues/2).

### 🧪 Tests

- Qualify deletion, replay and cancellation at the 4,096-label ceiling and final cache callbacks after controller revocation or a complete admission close/reopen cycle.
- Reproduce upgrades from the checksum-pinned published 0.2.2 executable and its packaged lockfile.
- Verify quota-rejected restoration preserves tombstones across reopen, and paginate removed labels across mixed names and versions without omissions or duplicates.
- Qualify upgrades while admission is closed: deletion, cancellation, replay and restoration remain fenced, and activation retains its exact maintenance owner.
- Exercise installed removal endpoint authorization, frame budgets, method/direction separation and conflicting-manifest refusal.

### 🔧 Tooling

- Add Linux/macOS hosted qualification and one full-history secret scan per CI run.
- Require successful hosted CI for the exact pushed source and verified Wasm archive provenance before Make publication, with explicit archive paths and failure-path admission tests.

## [0.2.2] - 2026-11-15

### 📚 Documentation

- Clarify fresh compiler setup, exact restoration and retained counters, simulator startup, and the checks maintainers must perform separately before publication.

## [0.2.1] - 2026-11-15

### 📚 Documentation

- Package-specific READMEs distinguish the deployable canister from its passive protocol, with usage, build instructions and registry-safe links. Registry descriptions, documentation links, keywords and categories improve discovery.

## [0.2.0] - 2026-11-15

### ✨ Added

- First crates.io release of `ic-wasm-store-protocol` and `ic-wasm-store`, with registry dependency metadata and publication checks.
- Exact-manifest deletion and upload cancellation reclaim unshared payloads while retaining permanent release identities. The same manifest can be restored; pinned artifacts remain protected.
- Removed manifests can be listed through the replicated tombstone endpoint. Tombstones retain label quota and are included in preservation enumeration.

### ⚠️ Changed

- Once tombstones are written, older executables cannot read those release keys. Consumers must account for tombstones when comparing active listings with retained release counts; payload reclamation does not shrink stable memory or clear the management cache.

### 🐛 Fixed

- Cache preparation checks controller authority again after its final upload reply.

### 🧪 Testing

- Installed qualification exercises management-cache population and reuse, maintenance closure and reopening, and delayed-close replay.
- Installed interleaving cases verify typed failures when admission closes or the initiating controller is revoked during outstanding cache work.
- Developer commands now offer separate protocol/state tests and native/Wasm/qualification lint targets, with their scope listed in `make help`.

## [0.1.0] - 2026-11-15

### ✨ Added

Standalone Store and passive protocol extracted from Canic v0.214.0, with supplier-owned native and installed qualification. Protocol and persistent formats are preserved; package names are now ic-wasm-store and ic-wasm-store-protocol.

### ▶️ Try it

After tool and simulator preparation:

```sh
make test
make build
make qualification-test
```
