# Changelog

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
