# IC Wasm Store Protocol

Passive Rust types, method identifiers and bounded IC Wire messages for
[IC Wasm Store](https://crates.io/crates/ic-wasm-store), an immutable Wasm artifact
store on the Internet Computer. This crate works in native clients and canisters;
it does not make network calls, manage stable memory or deploy code.

```toml
[dependencies]
ic-wasm-store-protocol = "0.9"
ic-wire = { version = "0.30.1", default-features = false }
```

A release is an application-defined name and version, bound to a SHA-256 digest,
byte length and ordered chunk hashes. The protocol covers resumable uploads,
publication, reads, quotas, operation pins, cache preparation, maintenance
admission, deletion, cancellation and exact restoration.

```rust
use ic_wasm_store_protocol::{Manifest, Release, digest};

let bytes = b"\0asm\x01\0\0\0";
let manifest = Manifest {
    release: Release { name: "worker".into(), version: "1.0.0".into() },
    hash: digest(bytes),
    bytes: bytes.len() as u64,
    chunks: vec![digest(bytes)],
};
assert!(manifest.valid());
```

`Manifest::valid` checks labels and transfer geometry. Store publication checks
content integrity and the raw Wasm header, not full module validity. Store
endpoints require a controller except for granted reads. Clients encode IC Wire frames using the exported
method identifiers and frame limits; this crate is not a transport client.

Artifacts are limited to 8 MiB in 1 MiB chunks. Deleted identities remain
reserved: only the exact original manifest can be restored. Retained pins block
removal. The protocol is experimental and requires Rust 1.100.0.

For closed maintenance scans, `PRESERVATION` (117) returns `preservation::Page`
with explicit `Entry::Active` and `Entry::Removed` values. Compare `Entry::order_key`
and use the returned release as the next cursor under the same closed attempt.
Preservation commitments must include the discriminator, not just the manifest.
Method 113 is retired.

For direct installation, a controller calls `grant_read` (118) with a
`ReadGrant` containing its retained pin, the installer's raw principal bytes,
the current open maintenance sequence (zero before the first attempt), and an
absolute deadline within five minutes. The installer calls `read_granted` (119)
with a `GrantedRead` containing the issuing controller's raw principal bytes,
the exact grant, and the chunk index. Encode the request with `LIMITS` and decode
the reply with `CHUNK_LIMITS`. Repeating an admitted read returns the same chunk
while the grant remains valid. A maintenance closure invalidates all grants,
including after activation. Expired grants are reclaimed on issuance. Store
admits at most 32 active grants.

- [API documentation](https://docs.rs/ic-wasm-store-protocol)
- [Source and development instructions](https://github.com/dragginzgame/ic-wasm-store)
- [Compatibility and removal semantics](https://github.com/dragginzgame/ic-wasm-store/blob/v0.9.0/docs/design/extraction.md)
