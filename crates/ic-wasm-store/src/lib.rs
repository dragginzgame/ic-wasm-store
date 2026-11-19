//! Versioned artifact custody with exact removal and restoration. No deployment authority.
//!
//! This crate supplies a standalone Internet Computer canister. Build for
//! `wasm32-unknown-unknown` to produce its controller-authorized IC exports.
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

    use crate::state::State;
    use ic_platform_runtime::{self as runtime, system};
    use ic_storage::DefaultMemoryImpl;
    use ic_wasm_store_protocol as protocol;
    use mount::endpoint;
    use std::{cell::RefCell, convert::Infallible, fmt::Debug};

    // init
    // Initializes a fresh Store with its four checked allocations.
    // scope  : IC-authorized installation
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
    // scope  : IC-authorized upgrade
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

    // prepare
    // Reserve quota and stage the exact release manifest.
    // scope  : controllers
    // limits : 16 KiB request and response frames, 8 MiB per artifact
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
    // Write a verified chunk and return upload progress.
    // scope  : controllers
    // limits : 260 KiB request frame with a 256 KiB chunk, 16 KiB response frame
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
    // Publish a complete upload after verifying its artifact digest.
    // scope  : controllers
    // limits : 16 KiB request and response frames, 8 MiB per artifact
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
    // scope  : controllers
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
    // scope  : controllers
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
    // scope  : controllers
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
    // Page through preservation custody under the captured maintenance attempt.
    // scope  : the controller that owns the exact closed maintenance attempt
    // limits : 16 KiB request and response frames, one entry per page
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

    // observe_preservation_v2
    // Page through explicit active and removed entries under the same closed attempt.
    // scope  : the controller that owns the exact closed maintenance attempt
    // limits : 16 KiB request and response frames, one entry per page
    endpoint!(
        observe_preservation_v2,
        "observe_preservation_v2",
        update,
        PRESERVATION_V2,
        protocol::preservation::Request,
        Result<protocol::preservation::PageV2, protocol::Error>,
        LIMITS,
        LIMITS,
        |state, input| state.preservation_v2(system::caller().as_slice(),
        input)
    );

    // admission
    // Observe or change maintenance admission for the exact attempt.
    // scope  : controllers, with exact attempt ownership required for activation
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
    // Query one verified chunk of a published release.
    // scope  : controllers
    // limits : 16 KiB request frame, 260 KiB response frame with a 256 KiB chunk
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
    // Read one verified chunk through replicated execution.
    // scope  : controllers
    // limits : 16 KiB request frame, 260 KiB response frame with a 256 KiB chunk
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

    // list
    // Query the next page of active releases.
    // scope  : controllers
    // limits : 16 KiB request and response frames, four releases per page
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
    // scope  : controllers
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
    // scope  : controllers
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
    // Apply quotas within the configured limits and current reservations.
    // scope  : controllers
    // limits : 16 KiB request and response frames, 256 MiB reserved and 4,096 releases maximum
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
    // Retain the exact operation against a published release.
    // scope  : controllers, with pins owned by the caller
    // limits : 16 KiB request and response frames, 32 retained pins per artifact
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
    // Observe the receipt for an exact retained operation.
    // scope  : controllers, restricted to the caller's exact pin
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
    // scope  : controllers
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
    // scope  : controllers
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
    // Read the next page of retained removed identities.
    // scope  : controllers
    // limits : 16 KiB request and response frames, four manifests per page
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
    // Prepares the management chunk cache for the artifact retained by the caller's exact pin.
    // scope  : controllers with the exact pin and a valid admission epoch
    // limits : 16 KiB request and response frames, deadline at most 300 seconds ahead
    #[runtime::update(
        runtime = runtime,
        name = "prepare_cache",
        guard = cache::admit,
        decode = cache::decode,
        encode = cache::encode,
        max_bytes = 16384,
        on_error = fail
    )]
    async fn prepare_cache(
        input: protocol::PrepareCache,
    ) -> Result<protocol::CacheReady, protocol::CacheError> {
        cache::prepare(input).await
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
    const _: () = assert!(protocol::CHUNK_LIMITS.bytes == 266_240);
    const _: () = assert!(protocol::LIMITS.bytes == 16_384);
}
