# IC Wasm Store

This repository adopts the reviewed baseline in [TOKO-TOOLING.md](TOKO-TOOLING.md).
Read it before edits; update managed files only through the authenticated installer.

- Store owns immutable Wasm releases, resumable uploads, quotas, pins, cache preparation, persistence and exact maintenance admission. It has no Canic dependency or deployment authority.
- Keep the protocol passive and Host-safe. Standalone entrypoints use Platform and Wire; Storage owns checked memory allocation.
- Preserve endpoint identities, encoded records, allocation names and retained maintenance fences. Reopen existing state; never initialize after a load failure.
- Canic owns Root orchestration and end-to-end fleet qualification in its separate repository. Do not copy it here.
- Native tests stay with their packages; installed tests live in the independent qualification workspace.
- Use Rust 1.100.0 and inherited workspace lints. Product packages are authorized for crates.io publication; qualification packages remain unpublished.
- Use make test, make clippy, make doc and make qualification-test for affected checks. Simulator preparation is explicit via make qualification-setup. make ci is the complete delivery gate.
- maintenance-candidate marks qualification only; do not select it for live deployment.
- No automatic browser or desktop launches.

Read [the extraction contract](docs/design/extraction.md) for provenance, compatibility and the boundary with Canic.
