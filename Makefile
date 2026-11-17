.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification
CARGO_PUBLISH_PACKAGES := ic-wasm-store-protocol ic-wasm-store
CARGO_PUBLISH_REGISTRY := crates-io
# Preserve the existing local release gate; hosted CI is asynchronous.
CARGO_PUBLISH_VERIFY_TARGETS := ci publication-admission
LOCAL_TOOL_INSTALL_TARGETS := install-testkit-launcher
LOCAL_TOOL_CHECK_TARGETS := testkit-launcher-check
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
	  'make qualification-setup   Admit the launcher; prepare PocketIC and the released archive' \
	  'make qualification-test    Test installed custody, cache and maintenance' \
	  'make qualification-predecessor-test  Test upgrade from published 0.2.2' \
	  'make test-publication      Test publication admission without uploads' \
	  'make test-script-paths     Test setup/build paths with CDPATH enabled' \
	  'make test-setup-evidence   Test CI setup failure collection and launcher selection' \
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
.PHONY: install-testkit-launcher testkit-launcher-check test-setup-evidence
install-testkit-launcher:
	bash scripts/testkit-launcher.sh setup > /dev/null
testkit-launcher-check:
	bash scripts/testkit-launcher.sh check > /dev/null
qualification-setup:
	bash scripts/prepare-server.sh
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
.PHONY: test-server-diagnostics
.PHONY: test-script-paths
test-script-paths:
	bash scripts/tests/path-resolution.sh
test-server-diagnostics:
	bash scripts/tests/server-setup-diagnostics.sh
test-setup-evidence:
	bash scripts/tests/setup-evidence.sh
ci: clippy
	$(MAKE) fmt-check toko-tooling-check test doc test-publication test-server-diagnostics test-script-paths test-setup-evidence qualification-test
	$(MAKE) qualification-predecessor-test
