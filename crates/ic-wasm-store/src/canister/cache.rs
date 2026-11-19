//! One bounded cache repair. Stable bytes and pins survive callback loss; cache writes are idempotent.

use super::{STATE, protocol, system};
use ic_platform_protocol::{candid, management_chunks};
use ic_platform_runtime::call::{Call, Outcome, Wait};
use ic_platform_types::Principal;
use protocol::{CacheError, CacheReady, PrepareCache, Read};

const REPLY_LIMITS: candid::Limits = candid::Limits {
    bytes: 65_536,
    header: 4096,
    types: 256,
    work: 131_072,
    allocation: 262_144,
    vector: 65_536,
    skipping: 131_072,
};

/// Require a controller and enforce the cache request budget.
pub(super) fn admit() -> Result<(), String> {
    if !system::is_controller(&system::caller()) {
        return Err("Store controller required".into());
    }
    if system::argument_size() > protocol::LIMITS.bytes {
        return Err("Store frame too large".into());
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8]) -> Result<PrepareCache, ic_wire::Error> {
    ic_wire::decode(
        bytes,
        ic_wire::Encoding::Frame {
            method: protocol::PREPARE_CACHE,
            direction: ic_wire::Direction::Request,
        },
        protocol::LIMITS,
    )
}

#[expect(clippy::needless_pass_by_value, reason = "Platform owns the reply")]
pub(super) fn encode(reply: Result<CacheReady, CacheError>) -> Result<Vec<u8>, ic_wire::Error> {
    ic_wire::encode(
        &reply,
        ic_wire::Encoding::Frame {
            method: protocol::PREPARE_CACHE,
            direction: ic_wire::Direction::Response,
        },
        protocol::LIMITS,
    )
}

fn authority(caller: &Principal, expires: u64, epoch: u64) -> Result<(), CacheError> {
    if !system::is_controller(caller) {
        return Err(CacheError::Authority);
    }
    if system::time() > expires {
        return Err(CacheError::Expired);
    }
    STATE
        .with_borrow(|state| state.as_ref().expect("Store state").cache_fence(epoch))
        .map_err(CacheError::Pin)
}

async fn management(method: &str, bytes: Vec<u8>) -> Result<Vec<u8>, CacheError> {
    let reply = Call::new(Principal::management_canister(), method, Wait::Unbounded)
        .owned_args(bytes)
        .execute()
        .await
        .map_err(|_| CacheError::Uncertain)?;
    match reply.into_parts().0 {
        Outcome::Reply(bytes) => Ok(bytes),
        Outcome::Rejected(rejection) | Outcome::OutcomeUnknown(rejection) => {
            Err(CacheError::Rejected(rejection.code()))
        }
    }
}

pub(super) async fn prepare(input: PrepareCache) -> Result<CacheReady, CacheError> {
    let caller = system::caller();
    let epoch = STATE
        .with_borrow(|state| state.as_ref().expect("Store state").admission_epoch())
        .map_err(CacheError::Pin)?;
    authority(&caller, input.expires_at_ns, epoch)?;
    if input.expires_at_ns.saturating_sub(system::time()) > 300_000_000_000 {
        return Err(CacheError::Invalid);
    }
    STATE
        .with_borrow(|state| {
            state
                .as_ref()
                .expect("Store state")
                .observe_pin(caller.as_slice(), &input.pin)
        })
        .map_err(CacheError::Pin)?;
    let canister_id = system::canister_self();
    let args =
        management_chunks::encode_stored_chunks(canister_id, 1024).expect("chunk observation args");
    let reply = management("stored_chunks", args).await?;
    let mut chunks = management_chunks::decode_stored_chunks(&reply, REPLY_LIMITS)
        .map_err(|_| CacheError::Uncertain)?;
    let mut uploaded = 0;
    for (index, hash) in input.pin.manifest.chunks.iter().enumerate() {
        authority(&caller, input.expires_at_ns, epoch)?;
        if chunks.iter().any(|chunk| chunk.hash == *hash) {
            continue;
        }
        let bytes = STATE
            .with_borrow(|state| {
                let state = state.as_ref().expect("Store state");
                state.observe_pin(caller.as_slice(), &input.pin)?;
                state.read(&Read {
                    release: input.pin.manifest.release.clone(),
                    index: index as u32,
                })
            })
            .map_err(CacheError::Pin)?
            .0;
        let args = management_chunks::encode_upload_chunk(
            canister_id,
            &bytes,
            protocol::CHUNK_LIMITS.bytes,
        )
        .expect("chunk upload args");
        let reply = management("upload_chunk", args).await?;
        let result = management_chunks::decode_upload_chunk(&reply, REPLY_LIMITS)
            .map_err(|_| CacheError::Uncertain)?;
        if result.hash != *hash {
            return Err(CacheError::Uncertain);
        }
        chunks.push(result);
        uploaded += 1;
    }
    // A close/reopen cycle fences even the final callback; it cannot report fresh readiness.
    if !system::is_controller(&caller) {
        return Err(CacheError::Authority);
    }
    STATE
        .with_borrow(|state| state.as_ref().expect("Store state").cache_fence(epoch))
        .map_err(CacheError::Pin)?;
    // Late completion remains evidence, never permission to install past the approval deadline.
    Ok(CacheReady {
        pin: input.pin,
        uploaded,
    })
}
