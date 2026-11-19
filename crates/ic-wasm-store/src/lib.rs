//! Versioned artifact custody with exact removal and restoration. No deployment authority.
//!
//! This crate supplies a standalone Internet Computer canister. Build for
//! `wasm32-unknown-unknown` to compile its controller-authorized entrypoints.
//! The native library exposes no service API. Clients use
//! [ic-wasm-store-protocol](https://docs.rs/ic-wasm-store-protocol) for passive
//! request/response types and IC Wire method identifiers.
//!
//! The `maintenance-candidate` feature identifies qualification binaries and
//! must not be selected for live deployment. See the
//! [repository](https://github.com/dragginzgame/ic-wasm-store) for build,
//! qualification and retained-state compatibility requirements.
#[cfg(target_arch = "wasm32")]
mod canister;

// Native unit tests exercise the canister's private state implementation.
// Ordinary host builds expose no state API and omit the storage dependencies.
#[cfg(any(target_arch = "wasm32", test))]
mod state;
