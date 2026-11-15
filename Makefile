.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification
include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk

.PHONY: help check test clippy doc build qualification-setup qualification-test ci
help:
	@printf '%s\n' 'make build                 Build the standalone Store Wasm' 'make test                  Run protocol and persistence tests' 'make qualification-setup   Explicitly prepare PocketIC' 'make qualification-test    Test installed upload/reopen/version reuse' 'make ci                    Run all checks'
check:
	cargo check --locked --workspace --all-targets
test:
	cargo test --locked --workspace
clippy:
	cargo clippy --locked --workspace --all-targets -- -D warnings
	cargo clippy --locked -p ic-wasm-store --target wasm32-unknown-unknown --all-features -- -D warnings
	cargo clippy --locked --manifest-path qualification/Cargo.toml --workspace --all-targets -- -D warnings
doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --manifest-path qualification/Cargo.toml --workspace --no-deps
build:
	cargo build --locked --release -p ic-wasm-store --target wasm32-unknown-unknown
qualification-setup:
	cargo run --locked --manifest-path qualification/Cargo.toml --example prepare_server
qualification-test:
	bash scripts/test-installed.sh
ci: fmt-check toko-tooling-check test clippy doc qualification-test
