//! Explicit lifecycle and controller-only publication/read endpoints.
mod cache;

use crate::state::State;
use ic_platform_runtime::{self as runtime, system};
use ic_storage::DefaultMemoryImpl;
use ic_wasm_store_protocol as protocol;
use std::{cell::RefCell, convert::Infallible, fmt::Debug};

thread_local! { static STATE: RefCell<Option<State<DefaultMemoryImpl>>> = const { RefCell::new(None) }; }

/// Initialize a fresh Store with its four checked allocations.
#[runtime::init(
    runtime = runtime,
    guard = lifecycle_admit,
    decode = lifecycle_decode,
    max_bytes = 16384,
    on_error = fail
)]
fn init((): ()) {
    STATE.with_borrow_mut(|state| *state = Some(State::open(DefaultMemoryImpl::default(), true)));
}

/// Reopen the existing Store after an upgrade.
#[runtime::post_upgrade(
    runtime = runtime,
    guard = lifecycle_admit,
    decode = lifecycle_decode,
    max_bytes = 16384,
    on_error = fail
)]
fn post_upgrade((): ()) {
    STATE.with_borrow_mut(|state| *state = Some(State::open(DefaultMemoryImpl::default(), false)));
}

// The IC authorizes lifecycle calls before running these hooks.
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

// Rustfmt 1.100 repeatedly reindents the interpolated Runtime attribute.
#[rustfmt::skip]
macro_rules! endpoint {
    ($name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr) => {
        mod $name {
            use super::{DefaultMemoryImpl, STATE, State, fail, protocol, runtime, system};

            // Require a controller and enforce this endpoint's request budget.
            fn admit() -> Result<(), String> {
                if !system::is_controller(&system::caller()) {
                    return Err("Store controller required".into());
                }
                if system::argument_size() > protocol::$input_limits.bytes {
                    return Err("Store frame too large".into());
                }
                Ok(())
            }

            fn decode(bytes: &[u8]) -> Result<$request, ic_wire::Error> {
                ic_wire::decode(
                    bytes,
                    ic_wire::Encoding::Frame {
                        method: protocol::$id,
                        direction: ic_wire::Direction::Request,
                    },
                    protocol::$input_limits,
                )
            }

            fn encode(reply: $reply) -> Result<Vec<u8>, ic_wire::Error> {
                ic_wire::encode(
                    &reply,
                    ic_wire::Encoding::Frame {
                        method: protocol::$id,
                        direction: ic_wire::Direction::Response,
                    },
                    protocol::$output_limits,
                )
            }

            #[runtime::$kind(
                runtime = runtime,
                name = $export,
                guard = admit,
                decode = decode,
                encode = encode,
                max_bytes = 266240,
                on_error = fail
            )]
            fn $name(input: $request) -> $reply {
                STATE.with_borrow_mut(|state| {
                    let operation: fn(&mut State<DefaultMemoryImpl>, &$request) -> $reply =
                        $operation;
                    operation(state.as_mut().expect("initialized Store"), &input)
                })
            }
        }
    };
}

// Reserve quota and stage the exact release manifest.
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

// Write a verified chunk and return upload progress.
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

// Publish a complete upload after verifying its artifact digest.
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

// Query the current status of an active release.
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

// Read active release status through replicated execution.
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

// Read upload progress and retained pins through replicated execution.
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

// Page through preservation custody under the captured maintenance attempt.
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

// Page through explicit active and removed entries under the same closed attempt.
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

// Observe or change maintenance admission for the exact attempt.
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

// Query one verified chunk of a published release.
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

// Read one verified chunk through replicated execution.
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

// Query the next page of active releases.
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

// Query Store quotas and usage.
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

// Read Store quotas and usage through replicated execution.
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

// Apply quotas within the configured limits and current reservations.
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

// Retain the exact operation against a published release.
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

// Observe the receipt for an exact retained operation.
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

// Remove an exact published release.
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

// Cancel an exact unpublished upload.
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

// Read the next page of retained removed identities.
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
