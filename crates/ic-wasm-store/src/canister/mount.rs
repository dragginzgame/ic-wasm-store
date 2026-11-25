//! Mount Store operations with their authority, Wire methods and frame budgets.

// Rustfmt 1.100 repeatedly reindents the interpolated Runtime attribute.
#[rustfmt::skip]
macro_rules! mount_endpoint {
    ([$($qualifier:tt)*], $controller:literal, $maximum:literal, $name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, |$input:ident| $body:block) => {
        mod $name {
            use super::{fail, protocol, runtime, system};

            /// Enforce the request budget and the controller gate when selected.
            fn admit() -> Result<(), String> {
                if $controller && !system::is_controller(&system::caller()) {
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
            // guard  : configured controller gate, followed by operation-specific admission
            // limits : $input_limits request and $output_limits response Wire budgets
            #[runtime::$kind(
                runtime = runtime,
                name = $export,
                guard = admit,
                decode = decode,
                encode = encode,
                max_bytes = $maximum,
                on_error = fail
            )]
            $($qualifier)* fn $name($input: $request) -> $reply $body
        }
    };
}

macro_rules! endpoint {
    ($($args:tt)*) => { mount::sync_endpoint!(true, $($args)*); };
}

macro_rules! granted_endpoint {
    ($($args:tt)*) => { mount::sync_endpoint!(false, $($args)*); };
}

macro_rules! sync_endpoint {
    ($controller:literal, $name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr) => {
        mount::mount_endpoint!(
            [],
            $controller,
            1052672,
            $name,
            $export,
            $kind,
            $id,
            $request,
            $reply,
            $input_limits,
            $output_limits,
            |input| {
                use super::{DefaultMemoryImpl, STATE, State};

                STATE.with_borrow_mut(|state| {
                    let operation: fn(&mut State<DefaultMemoryImpl>, &$request) -> $reply =
                        $operation;
                    operation(state.as_mut().expect("initialized Store"), &input)
                })
            }
        );
    };
}

macro_rules! async_endpoint {
    ($name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr) => {
        mount::mount_endpoint!([async], true, 16384, $name, $export, $kind, $id, $request, $reply, $input_limits, $output_limits, |input| {
            $operation(input).await
        });
    };
}

macro_rules! endpoints {
    ($(
        $mount:ident!($name:ident, $export:literal, $kind:ident, $id:ident, $request:ty, $reply:ty, $input_limits:ident, $output_limits:ident, $operation:expr);
    )*) => {
        $(mount::$mount!($name, $export, $kind, $id, $request, $reply, $input_limits, $output_limits, $operation);)*

        fn ingress_size_allowed(method: &str, bytes: usize) -> bool {
            match method {
                $($export => bytes <= protocol::$input_limits.bytes,)*
                _ => false,
            }
        }

        #[cfg(test)]
        fn wire_endpoints() -> Vec<wire_api::Endpoint> {
            vec![$(
                wire_api::Endpoint::new::<$request, $reply>(
                    $export,
                    stringify!($kind),
                    protocol::$id,
                    protocol::$input_limits,
                    protocol::$output_limits,
                ),
            )*]
        }
    };
}

pub(super) use {
    async_endpoint, endpoint, endpoints, granted_endpoint, mount_endpoint, sync_endpoint,
};
