# IC Wasm Store

A standalone, controller-authorized Internet Computer canister for immutable, versioned Wasm custody. It retains resumable uploads, shared chunks, quotas and operation pins; it prepares the management chunk cache without installing application code.

Canic is its first consumer. This repository has no Canic runtime, Host or CLI dependency. Deployment approvals, controllers and fleet orchestration stay with the consuming application.

## Development

Use Rust 1.100.0 with the wasm32-unknown-unknown target. Host workflows support Linux and macOS; initial extraction evidence is Linux only.

```sh
make install-tools
make tools-check
make test
make build
make qualification-setup
make qualification-test
```

The standalone Wasm is built from ic-wasm-store; ic-wasm-store-protocol contains its passive Wire contract. Cargo.lock pins dependencies. All packages are publish=false.

See [ownership and preservation](docs/design/extraction.md) and [qualification](qualification/README.md). This remains experimental: finite histories, controller-only access, 8 MiB artifact verification bound and no deletion or deployment engine. Availability of older code does not imply safe application-state rollback.
