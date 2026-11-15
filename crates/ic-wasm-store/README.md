# IC Wasm Store

A standalone Internet Computer canister for immutable, versioned Wasm artifacts.
It provides controller-authorized resumable uploads, shared chunks, integrity
verification, quotas, retained operation pins and management-cache preparation.
It stores and prepares code; the consuming application owns deployment approvals
and installation.

This package supplies the canister implementation. For request/response types in
native clients or other canisters, use
[ic-wasm-store-protocol](https://crates.io/crates/ic-wasm-store-protocol).
The native library exposes no Store service API; canister entrypoints compile
only for `wasm32-unknown-unknown`.

## Build

With Git, rustup and GNU Make installed, use the tagged source and its committed
lockfile for a reproducible dependency selection:

```sh
git clone --branch v0.2.2 https://github.com/dragginzgame/ic-wasm-store.git
cd ic-wasm-store
rustup toolchain install 1.100.0 --profile minimal --component rustfmt,clippy --target wasm32-unknown-unknown
make build
```

The build produces `target/wasm32-unknown-unknown/release/ic_wasm_store.wasm`.
It does not install or upgrade a canister. The `maintenance-candidate` feature
is for qualification only; do not enable it for live deployment.

## Contract

Release names and versions bind permanently to exact content. Controllers can
delete published releases or cancel unpublished uploads, then restore the same
manifest. Different bytes require a new version. Pinned artifacts cannot be
removed. Unshared payloads are reclaimed, while tombstones continue consuming
release-label quota. Removal does not shrink stable memory or clear management
cache copies.

Restoration replaces the tombstone with an active release and rechecks payload
quota. Missing bytes must be uploaded and publication repeated unless an active
alias already retains the published artifact. Ordinary status/list calls expose
active releases; the tombstone endpoint lists removed identities separately.

Artifacts are limited to 8 MiB in 256 KiB chunks. Publication verifies hashes and
the raw Wasm header, not full module validity. All endpoints require a controller.
Closing maintenance admission does not drain outstanding calls. Consumers must
settle outstanding removal requests before restoring a label, and executables
predating tombstones cannot reopen state after tombstones are written.

This is experimental software requiring Rust 1.100.0. Registry publication is
not deployment qualification; native macOS and some callback schedules remain
unqualified.

- [Protocol API](https://docs.rs/ic-wasm-store-protocol)
- [Source and development instructions](https://github.com/dragginzgame/ic-wasm-store)
- [Compatibility and removal semantics](https://github.com/dragginzgame/ic-wasm-store/blob/v0.2.2/docs/design/extraction.md)
- [Qualification coverage](https://github.com/dragginzgame/ic-wasm-store/blob/v0.2.2/qualification/README.md)
