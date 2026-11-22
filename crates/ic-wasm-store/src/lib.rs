//! Versioned artifact custody with exact removal and restoration. No deployment authority.
//!
//! This crate supplies a standalone Internet Computer canister. Build for
//! `wasm32-unknown-unknown` to produce its IC exports.
//! The native library exposes no service API. Clients use
//! [ic-wasm-store-protocol](https://docs.rs/ic-wasm-store-protocol) for passive
//! request/response types and IC Wire method identifiers.
//!
//! The `maintenance-candidate` feature identifies qualification binaries and
//! must not be selected for live deployment. See the
//! [repository](https://github.com/dragginzgame/ic-wasm-store) for build,
//! qualification and retained-state compatibility requirements.

mod state;

mod canister {
    mod cache;
    mod mount;
    #[cfg(test)]
    mod wire_api;

    use crate::state::State;
    use ic_platform_runtime::{self as runtime, system};
    use ic_storage::DefaultMemoryImpl;
    use ic_wasm_store_protocol as protocol;
    use mount::endpoints;
    use std::{cell::RefCell, convert::Infallible, fmt::Debug};

    // init
    // Initializes a fresh Store with its four checked allocations.
    // guard  : lifecycle_admit, with installation authorized by the IC
    // limits : 16 KiB lifecycle arguments
    #[runtime::init(
        runtime = runtime,
        guard = lifecycle_admit,
        decode = lifecycle_decode,
        max_bytes = 16384,
        on_error = fail
    )]
    fn init((): ()) {
        STATE.with_borrow_mut(|state| {
            *state = Some(State::open(DefaultMemoryImpl::default(), true))
        });
    }

    // post_upgrade
    // Reopens the existing Store after an upgrade.
    // guard  : lifecycle_admit, with upgrade authorized by the IC
    // limits : 16 KiB lifecycle arguments
    #[runtime::post_upgrade(
        runtime = runtime,
        guard = lifecycle_admit,
        decode = lifecycle_decode,
        max_bytes = 16384,
        on_error = fail
    )]
    fn post_upgrade((): ()) {
        STATE.with_borrow_mut(|state| {
            *state = Some(State::open(DefaultMemoryImpl::default(), false))
        });
    }

    endpoints! {
        // prepare
        // Reserves quota and stages the exact manifest for an artifact up to 8 MiB.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            prepare,
            "prepare",
            update,
            PREPARE,
            protocol::Manifest,
            Result<protocol::Status, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.prepare(input)
        );

        // put
        // Writes one verified chunk of up to 1 MiB and returns upload progress.
        // guard  : canister controller
        // limits : 1028 KiB request frame, 16 KiB response frame
        endpoint!(
            put,
            "put",
            update,
            PUT,
            protocol::Put,
            Result<protocol::Status, protocol::Error>,
            CHUNK_LIMITS,
            LIMITS,
            |state, input| state.put(input)
        );

        // publish
        // Publishes a complete artifact of up to 8 MiB after verifying its digest.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            publish,
            "publish",
            update,
            PUBLISH,
            protocol::Release,
            Result<protocol::Status, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.publish(input)
        );

        // status
        // Query the current status of an active release.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            status,
            "status",
            query,
            STATUS,
            protocol::Release,
            Result<protocol::Status, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.status(input)
        );

        // observe
        // Read active release status through replicated execution.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            observe,
            "observe",
            update,
            STATUS,
            protocol::Release,
            Result<protocol::Status, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.status(input)
        );

        // observe_custody
        // Read upload progress and retained pins through replicated execution.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            observe_custody,
            "observe_custody",
            update,
            CUSTODY,
            protocol::Release,
            Result<protocol::Custody, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.custody(input)
        );

        // observe_preservation
        // Returns one active or removed entry for the controller that owns the exact closed attempt.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            observe_preservation,
            "observe_preservation",
            update,
            PRESERVATION,
            protocol::preservation::Request,
            Result<protocol::preservation::Page, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.preservation(system::caller().as_slice(),
            input)
        );

        // admission
        // Observes or changes exact maintenance admission. Activation requires the captured owner.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            admission,
            "admission",
            update,
            ADMISSION,
            protocol::AdmissionCommand,
            Result<Option<protocol::Admission>, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.admission(system::caller().as_slice(), input)
        );

        // read
        // Queries one verified chunk of up to 1 MiB from a published release.
        // guard  : canister controller
        // limits : 16 KiB request frame, 1028 KiB response frame
        endpoint!(
            read,
            "read",
            query,
            READ,
            protocol::Read,
            Result<protocol::Chunk, protocol::Error>,
            LIMITS,
            CHUNK_LIMITS,
            |state, input| state.read(input)
        );

        // read_chunk
        // Reads one verified chunk of up to 1 MiB through replicated execution.
        // guard  : canister controller
        // limits : 16 KiB request frame, 1028 KiB response frame
        endpoint!(
            read_chunk,
            "read_chunk",
            update,
            READ,
            protocol::Read,
            Result<protocol::Chunk, protocol::Error>,
            LIMITS,
            CHUNK_LIMITS,
            |state, input| state.read(input)
        );

        // grant_read
        // Authorizes a named installer to read one exact caller-owned pin.
        endpoint!(
            grant_read,
            "grant_read",
            update,
            GRANT_READ,
            protocol::ReadGrant,
            Result<(), protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.grant_read(system::caller().as_slice(), system::time(), input)
        );

        // read_granted
        // Reads one verified chunk when the caller proves the exact active grant.
        granted_endpoint!(
            read_granted,
            "read_granted",
            update,
            READ_GRANTED,
            protocol::GrantedRead,
            Result<protocol::Chunk, protocol::Error>,
            LIMITS,
            CHUNK_LIMITS,
            |state, input| {
                let owner = ic_platform_types::Principal::try_from_slice(&input.owner)
                    .map_err(|_| protocol::Error::NotFound)?;
                if !system::is_controller(&owner) {
                    return Err(protocol::Error::NotFound);
                }
                state.read_granted(system::caller().as_slice(), system::time(), input)
            }
        );

        // list
        // Queries the next page of up to four active releases.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            list,
            "list",
            query,
            LIST,
            Option<protocol::Release>,
            Result<Vec<protocol::Status>, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.list(input.as_ref())
        );

        // info
        // Query Store quotas and usage.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            info,
            "info",
            query,
            INFO,
            (),
            protocol::Info,
            LIMITS,
            LIMITS,
            |state, _| state.info()
        );

        // observe_info
        // Read Store quotas and usage through replicated execution.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            observe_info,
            "observe_info",
            update,
            INFO,
            (),
            protocol::Info,
            LIMITS,
            LIMITS,
            |state, _| state.info()
        );

        // configure
        // Applies quotas up to 256 MiB and 4,096 releases, subject to current reservations.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            configure,
            "configure",
            update,
            CONFIGURE,
            protocol::Quota,
            Result<protocol::Info, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.configure(input)
        );

        // pin
        // Retains the caller's exact operation against a published release, up to 32 pins per artifact.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            pin,
            "pin",
            update,
            PIN,
            protocol::Pin,
            Result<protocol::PinReceipt, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.pin(system::caller().as_slice(),
            input)
        );

        // observe_pin
        // Returns the receipt for the caller's exact retained pin.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            observe_pin,
            "observe_pin",
            update,
            OBSERVE_PIN,
            protocol::Pin,
            Result<protocol::PinReceipt, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.observe_pin(system::caller().as_slice(),
            input)
        );

        // delete
        // Remove an exact published release.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            delete,
            "delete",
            update,
            DELETE,
            protocol::Manifest,
            Result<protocol::Manifest, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.remove_release(input, true)
        );

        // cancel
        // Cancel an exact unpublished upload.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            cancel,
            "cancel",
            update,
            CANCEL,
            protocol::Manifest,
            Result<protocol::Manifest, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.remove_release(input, false)
        );

        // tombstones
        // Returns the next page of up to four removed manifests.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        endpoint!(
            tombstones,
            "tombstones",
            update,
            TOMBSTONES,
            Option<protocol::Release>,
            Result<Vec<protocol::Manifest>, protocol::Error>,
            LIMITS,
            LIMITS,
            |state, input| state.tombstones(input.as_ref())
        );

        // prepare_cache
        // Prepares the caller's exact pinned artifact under current admission with a deadline at most 300 seconds ahead.
        // guard  : canister controller
        // limits : 16 KiB request and response frames
        async_endpoint!(
            prepare_cache,
            "prepare_cache",
            update,
            PREPARE_CACHE,
            protocol::PrepareCache,
            Result<protocol::CacheReady, protocol::CacheError>,
            LIMITS,
            LIMITS,
            super::cache::prepare
        );
    }

    thread_local! { static STATE: RefCell<Option<State<DefaultMemoryImpl>>> = const { RefCell::new(None) }; }

    /// The IC authorizes lifecycle calls before running these hooks.
    fn lifecycle_admit() -> Result<(), Infallible> {
        Ok(())
    }

    fn lifecycle_decode(_bytes: &[u8]) -> Result<(), Infallible> {
        Ok(())
    }

    fn fail(error: impl Debug) -> ! {
        system::trap(&format!("Store entrypoint: {error:?}"))
    }

    // Each concrete declaration owns its access, frame budgets and protocol identity.
    // Platform owns IC entrypoints; Wire owns codecs. No lifecycle is generated here.
    // Platform's attributes require literal byte ceilings. The guard enforces each
    // endpoint's narrower protocol limit before Runtime captures any argument bytes.
    const _: () = assert!(protocol::CHUNK_LIMITS.bytes == 1_052_672);
    const _: () = assert!(protocol::LIMITS.bytes == 16_384);
}
