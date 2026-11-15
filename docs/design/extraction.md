# Extraction from Canic

Source: dragginzgame/canic at c39aee1e34dae82be6da89bf6242c3a0e22ade84 (v0.214.0), crates/canic-wasm-store and crates/canic-wasm-store-protocol. The source remains preserved in that repository's Git history.

The packages are now ic-wasm-store and ic-wasm-store-protocol. Rust import names change; the initial extraction otherwise preserves runtime and protocol source. Endpoint method names/numbers, Wire shapes, record magic, Storage authority/key names, quotas and maintenance sequence semantics are unchanged. New compilation produces a new executable identity: deployed upgrades still need explicit approval and qualification, never an implicit replacement.

Store owns release/chunk identity, verification, quota reservations, retained pins, cache reconstruction, admission closure and preservation observations. Controllers supply exact operation commitments; controller membership grants no authority in a consuming application's user model. Root owns deployment approvals, management effects and recovery in Canic. Its single retained Store-maintenance operation remains a Canic limitation; extraction does not implement repeatable Root maintenance.

Reused legacy research is recorded in Canic's docs/qualification/history.md at the source revision, section Versioned Wasm Store decision, inspecting canic-legacy 377fbceb3994c9d1f3033979ef4e4ce9e6b85801. Keep immutable versions, authoritative stable bytes and reconstructible management cache; do not import historical mutable-version replacement or the legacy control plane.

Supplier tests own persistence, metadata budgets and standalone installed behavior. Canic retains Root-mediated publication, interrupted management effects, preservation custody and complete CLI fleet journeys. Native and installed tests move with their owning behavior; no Canic dependency is introduced merely to run supplier checks.

Canic consumes an immutable Git revision of the protocol and builds the deployable Store from that same revision and its committed lockfile. No sibling checkout, path override, copied implementation or registry publication is required.

## Initial extraction evidence (2026-11-15)

Linux: 10 Store state tests and one protocol budget test pass, along with native/Wasm Clippy and both workspaces' Rustdoc, formatting and tooling admission. The standalone installed witness passes with the extracted original/candidate pair and with Canic's retained pre-extraction executable (SHA-256 `5bf18e1ee35c8c84ebaba44d713cd87bc8759dfe6bb033f707307748ca21df25`) upgraded to the extracted candidate. Both preserve and resume an 8 MiB upload, retain two versions and reconstruct exact bytes. Logs remain under `target/qualification/extraction-*`. This does not qualify mainnet deployment, a new maintenance sequence, or native macOS execution.
