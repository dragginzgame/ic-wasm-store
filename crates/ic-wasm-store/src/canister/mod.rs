//! Explicit lifecycle and controller-only publication/read endpoints.
use crate::state::State;
use ic_platform_runtime::{self as runtime, system};
use ic_storage::DefaultMemoryImpl;
use ic_wasm_store_protocol as protocol;
use std::cell::RefCell;
use std::{convert::Infallible, fmt::Debug};
mod cache;
thread_local! { static STATE: RefCell<Option<State<DefaultMemoryImpl>>> = const { RefCell::new(None) }; }
#[runtime::init(runtime = runtime, guard = lifecycle_admit, decode = lifecycle_decode, max_bytes = 16384, on_error = fail)]
fn init((): ()) {
    STATE.with_borrow_mut(|state| *state = Some(State::open(DefaultMemoryImpl::default(), true)));
}
#[runtime::post_upgrade(runtime = runtime, guard = lifecycle_admit, decode = lifecycle_decode, max_bytes = 16384, on_error = fail)]
fn post_upgrade((): ()) {
    STATE.with_borrow_mut(|state| *state = Some(State::open(DefaultMemoryImpl::default(), false)));
}

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
macro_rules! endpoint {
    ($name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr) => {
        mod $name {
            use super::{DefaultMemoryImpl, STATE, State, fail, protocol, runtime, system};
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
                        runtime = runtime, name = $export, guard = admit,
                        decode = decode, encode = encode, max_bytes = 266240, on_error = fail
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
endpoint!(prepare, "prepare", update, PREPARE, protocol::Manifest, Result<protocol::Status, protocol::Error>, LIMITS, LIMITS, |state, input| state.prepare(input));
endpoint!(put, "put", update, PUT, protocol::Put, Result<protocol::Status, protocol::Error>, CHUNK_LIMITS, LIMITS, |state, input| state.put(input));
endpoint!(publish, "publish", update, PUBLISH, protocol::Release, Result<protocol::Status, protocol::Error>, LIMITS, LIMITS, |state, input| state.publish(input));
endpoint!(status, "status", query, STATUS, protocol::Release, Result<protocol::Status, protocol::Error>, LIMITS, LIMITS, |state, input| state.status(input));
endpoint!(observe, "observe", update, STATUS, protocol::Release, Result<protocol::Status, protocol::Error>, LIMITS, LIMITS, |state, input| state.status(input));
endpoint!(observe_custody, "observe_custody", update, CUSTODY, protocol::Release, Result<protocol::Custody, protocol::Error>, LIMITS, LIMITS, |state, input| state.custody(input));
endpoint!(observe_preservation, "observe_preservation", update, PRESERVATION, protocol::preservation::Request, Result<protocol::preservation::Page, protocol::Error>, LIMITS, LIMITS, |state, input| state.preservation(system::caller().as_slice(), input));
endpoint!(observe_preservation_v2, "observe_preservation_v2", update, PRESERVATION_V2, protocol::preservation::Request, Result<protocol::preservation::PageV2, protocol::Error>, LIMITS, LIMITS, |state, input| state.preservation_v2(system::caller().as_slice(), input));
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
endpoint!(read, "read", query, READ, protocol::Read, Result<protocol::Chunk, protocol::Error>, LIMITS, CHUNK_LIMITS, |state, input| state.read(input));
endpoint!(read_chunk, "read_chunk", update, READ, protocol::Read, Result<protocol::Chunk, protocol::Error>, LIMITS, CHUNK_LIMITS, |state, input| state.read(input));
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
endpoint!(configure, "configure", update, CONFIGURE, protocol::Quota, Result<protocol::Info, protocol::Error>, LIMITS, LIMITS, |state, input| state.configure(input));
endpoint!(pin, "pin", update, PIN, protocol::Pin, Result<protocol::PinReceipt, protocol::Error>, LIMITS, LIMITS, |state, input| state.pin(system::caller().as_slice(), input));
endpoint!(observe_pin, "observe_pin", update, OBSERVE_PIN, protocol::Pin, Result<protocol::PinReceipt, protocol::Error>, LIMITS, LIMITS, |state, input| state.observe_pin(system::caller().as_slice(), input));
endpoint!(delete, "delete", update, DELETE, protocol::Manifest, Result<protocol::Manifest, protocol::Error>, LIMITS, LIMITS, |state, input| state.remove_release(input, true));
endpoint!(cancel, "cancel", update, CANCEL, protocol::Manifest, Result<protocol::Manifest, protocol::Error>, LIMITS, LIMITS, |state, input| state.remove_release(input, false));
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
