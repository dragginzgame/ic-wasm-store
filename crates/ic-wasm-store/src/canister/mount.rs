//! Mount a controller-only Store operation with its Wire method and frame budgets.

// Rustfmt 1.100 repeatedly reindents the interpolated Runtime attribute.
#[rustfmt::skip]
macro_rules! endpoint {
    ($name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr) => {
        mod $name {
            use super::{DefaultMemoryImpl, STATE, State, fail, protocol, runtime, system};

            /// Require a controller and enforce this endpoint's request budget.
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

            // $name
            // The mount supplies the caller-visible behavior.
            // scope  : controllers, subject to the operation's admission rules
            // limits : $input_limits request and $output_limits response Wire budgets
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

pub(super) use endpoint;
