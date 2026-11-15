.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification
CARGO_PUBLISH_PACKAGES := ic-wasm-store-protocol ic-wasm-store
CARGO_PUBLISH_REGISTRY := crates-io
CARGO_PUBLISH_VERIFY_TARGETS := toko-tooling-check publication-admission
include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk

.PHONY: help check test clippy doc build qualification-setup qualification-test ci
.PHONY: test-protocol test-store clippy-native clippy-wasm clippy-qualification
help:
	@printf '%s\n' \
	  'make build                 Build the standalone Store Wasm' \
	  'make check                 Check native product targets (not the Wasm entrypoints)' \
	  'make test                  Run all product tests' \
	  'make test-protocol         Run protocol budget tests' \
	  'make test-store            Run Store persistence and admission tests' \
	  'make clippy                Lint native, Wasm and qualification targets' \
	  'make clippy-native         Lint native product targets' \
	  'make clippy-wasm           Lint Store Wasm with all features' \
	  'make clippy-qualification  Lint the qualification workspace' \
	  'make doc                   Check documentation for both workspaces' \
	  'make qualification-setup   Prepare PocketIC and the released archive' \
	  'make qualification-test    Test installed custody, cache and maintenance' \
	  'make qualification-predecessor-test  Test upgrade from published 0.2.2' \
	  'make test-publication      Test publication admission without uploads' \
	  'make ci                    Run aggregate checks; requires prepared fixtures'
check:
	cargo check --locked --workspace --all-targets
test: test-protocol test-store
test-protocol:
	cargo test --locked -p ic-wasm-store-protocol
test-store:
	cargo test --locked -p ic-wasm-store
clippy: clippy-native clippy-wasm clippy-qualification
clippy-native:
	cargo clippy --locked --workspace --all-targets -- -D warnings
clippy-wasm:
	cargo clippy --locked -p ic-wasm-store --target wasm32-unknown-unknown --all-features -- -D warnings
clippy-qualification:
	cargo clippy --locked --manifest-path qualification/Cargo.toml --workspace --all-targets -- -D warnings
doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --manifest-path qualification/Cargo.toml --workspace --no-deps
build:
	cargo build --locked --release -p ic-wasm-store --target wasm32-unknown-unknown
qualification-setup:
	cargo run --locked --manifest-path qualification/Cargo.toml --example prepare_server
	bash scripts/prepare-predecessor.sh setup
qualification-test:
	bash scripts/test-installed.sh $(QUALIFICATION_TEST)
.PHONY: qualification-predecessor-test publication-admission test-publication
qualification-predecessor-test:
	bash scripts/prepare-predecessor.sh build
	IC_WASM_STORE_WASM="$(CURDIR)/target/qualification/predecessor.wasm" bash scripts/test-installed.sh versioned_upload_resumes
publication-admission:
	bash scripts/check-publication.sh
test-publication:
	bash scripts/tests/publication.sh
ci: fmt-check toko-tooling-check test clippy doc test-publication qualification-test
	$(MAKE) qualification-predecessor-test
