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

// Native tests cannot compile Platform Runtime's IC system and call execution APIs.
#[cfg(target_arch = "wasm32")]
mod canister;

#[cfg_attr(
    not(any(target_arch = "wasm32", test)),
    expect(dead_code, reason = "ordinary host builds have no Store entrypoints")
)]
mod state;
