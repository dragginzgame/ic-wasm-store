# Changelog

## [0.3.1] - 2026-11-17

### 🩹 Fixed

- Resolve local setup, qualification and publication script paths independently of the caller's `CDPATH`, preserving correct paths and exit statuses for relative invocations.

### 🔧 Changed

- Adopt Platform Protocol 0.3.2 for bounded management-cache messages. Store's selected chunk codecs and public Wire contract remain compatible.
- Refresh compatible lock selections to Platform Runtime 0.23.15, Wire 0.24.3 and Testkit 0.107.4. Retain the explicitly frozen 0.106.0 server-launcher bootstrap; migration to Testkit's newer optional launcher is deferred to the next server-tooling adoption.

## [0.3.0] - 2026-11-16

### ⚠️ Breaking

- Adopt Platform Runtime 0.23.14 and Protocol 0.2.6 for explicit entrypoints and bounded management-cache messages. Controller and oversized-frame refusals now trap (IC reject code 5), replacing the old guard rejection (code 4); callers must treat both as refused requests. Lifecycle arguments are bounded to 16 KiB. Stop and await Stopped before upgrading, then restart. Endpoint names, Wire records and retained Store state are unchanged.

### 🔧 Changed

- Remove the legacy Platform facade from the product graph. Storage 0.74.1 still owns the old Runtime's raw stable-memory adapter; only the new Runtime supplies the executor. Complete removal remains tracked in [Storage #3 — Wasm Store retained-state migration](https://github.com/dragginzgame/ic-storage/issues/3).

### 🔧 Tooling

- Adopt Toko Tooling 0.44.11 through the authenticated installer. Select compatible `thiserror` versions independently in the product and PocketIC qualification workspaces, without a fleet-wide hold.
- Adopt Testkit 0.107.3 for qualification clients, removing its legacy Platform/Host library graph. Server provisioning and command lifetime use the explicitly frozen 0.106.0 launcher outside the qualification graph; callers can also supply an externally owned server URL.

### 🩹 Fixed

- Keep hosted CI asynchronous during publication while enforcing the existing local
  validation gate and clean, unchanged source/archive provenance checks.

- Expose bounded, escaped simulator setup diagnostics locally and in CI, retain the original evidence for seven days, and preserve failure phase/status. CI reports the retained artifact link or missing-upload outcome. Qualify the real failed-download path without network access. Fixes [WasmStore #3 — Move PocketIC preparation diagnostics out of the legacy Testkit library setup call](https://github.com/dragginzgame/ic-wasm-store/issues/3).

## [0.2.4] - 2026-11-16

### 🔧 Tooling

- Adopt Toko Tooling 0.44.5 in the product and qualification workspaces through the authenticated snapshot installer.
- Document and qualify the repository-local shared formatting hook setup for fresh checkouts. Fixes [#1](https://github.com/dragginzgame/ic-wasm-store/issues/1).

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
