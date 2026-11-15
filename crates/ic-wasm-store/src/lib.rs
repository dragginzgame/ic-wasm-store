//! Versioned artifact custody with exact removal and restoration. No deployment authority.
#[cfg(target_arch = "wasm32")]
mod canister;
#[cfg(any(target_arch = "wasm32", test))]
mod state;
