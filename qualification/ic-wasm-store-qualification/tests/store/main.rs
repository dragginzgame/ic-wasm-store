//! Installed Store upload custody, raw chunk integrity and independent maintenance.
mod callbacks;
mod capacity;
mod maintenance;
mod read_grants;
mod support;
mod upload;

use ic_testkit::pocket_ic::{CanisterId, nonblocking::PocketIc};
use ic_wasm_store_protocol as store;
use ic_wire::{
    Encoding, Limits, Message,
    evolution::inline::{Fixed, Inline},
};
use serde::{Serialize, de::DeserializeOwned};

const fn wire_limits(budget: store::FrameBudget) -> ic_wire::Limits {
    ic_wire::Limits {
        bytes: budget.bytes,
        allocation: budget.allocation,
        nodes: budget.nodes,
        depth: budget.depth,
    }
}

struct Client<'a> {
    pic: &'a PocketIc,
    id: CanisterId,
    operator: CanisterId,
}

impl Client<'_> {
    async fn update<T: Clone + Fixed + Serialize + DeserializeOwned, R: for<'de> Inline<'de>>(
        &self,
        method: &str,
        id: u64,
        input: &T,
        limits: store::FrameBudget,
    ) -> R {
        let reply = self
            .pic
            .update_call(
                self.id,
                self.operator,
                method,
                encode_frame(
                    input,
                    ic_wire::Encoding::Frame {
                        method: id,
                        direction: ic_wire::Direction::Request,
                    },
                    crate::wire_limits(limits),
                )
                .unwrap(),
            )
            .await
            .unwrap();

        decode_frame(
            &reply,
            ic_wire::Encoding::Frame {
                method: id,
                direction: ic_wire::Direction::Response,
            },
            crate::wire_limits(limits),
        )
        .unwrap()
    }

    async fn inspect(&self, request: &store::Inspect) -> store::Inspection {
        let reply = self
            .pic
            .query_call(
                self.id,
                self.operator,
                "inspect",
                frame(store::INSPECT, request),
            )
            .await
            .unwrap();

        response::<Result<store::Inspection, store::Error>>(store::INSPECT, &reply).unwrap()
    }

    async fn info(&self) -> store::Info {
        let store::Inspection::Info(info) = self.inspect(&store::Inspect::Info).await else {
            panic!("expected Store info")
        };
        info
    }

    async fn observe(&self, request: &store::Observe) -> Result<store::Observation, store::Error> {
        self.update("observe", store::OBSERVE, request, store::LIMITS)
            .await
    }

    async fn status(&self, release: &store::Release) -> store::Status {
        let store::Observation::Status(status) = self
            .observe(&store::Observe::Status(release.clone()))
            .await
            .unwrap()
        else {
            panic!("expected release status")
        };
        status
    }
}

fn frame<T: Clone + Fixed + Serialize + DeserializeOwned>(method: u64, input: &T) -> Vec<u8> {
    encode_frame(
        input,
        ic_wire::Encoding::Frame {
            method,
            direction: ic_wire::Direction::Request,
        },
        crate::wire_limits(store::LIMITS),
    )
    .unwrap()
}

fn response<T: for<'de> Inline<'de>>(method: u64, bytes: &[u8]) -> T {
    decode_frame(
        bytes,
        ic_wire::Encoding::Frame {
            method,
            direction: ic_wire::Direction::Response,
        },
        crate::wire_limits(store::LIMITS),
    )
    .unwrap()
}

fn encode_frame<T: Clone + Fixed + Serialize + DeserializeOwned>(
    value: &T,
    encoding: Encoding,
    limits: Limits,
) -> Result<Vec<u8>, ic_wire::Error> {
    ic_wire::encode(&Message(value.clone()), encoding, limits)
}

fn decode_frame<'de, T: Inline<'de>>(
    bytes: &'de [u8],
    encoding: Encoding,
    limits: Limits,
) -> Result<T, ic_wire::Error> {
    ic_wire::decode::<Message<T>>(bytes, encoding, limits).map(|message| message.0)
}
