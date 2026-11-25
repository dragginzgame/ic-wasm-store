//! Installed Store upload custody, raw chunk integrity and independent maintenance.
mod callbacks;
mod capacity;
mod maintenance;
mod read_grants;
mod support;
mod upload;

use ic_testkit::pocket_ic::{CanisterId, nonblocking::PocketIc};
use ic_wasm_store_protocol as store;
use serde::{Serialize, de::DeserializeOwned};

struct Client<'a> {
    pic: &'a PocketIc,
    id: CanisterId,
    operator: CanisterId,
}

impl Client<'_> {
    async fn update<T: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        id: u64,
        input: &T,
        limits: ic_wire::Limits,
    ) -> R {
        let reply = self
            .pic
            .update_call(
                self.id,
                self.operator,
                method,
                ic_wire::encode(
                    input,
                    ic_wire::Encoding::Frame {
                        method: id,
                        direction: ic_wire::Direction::Request,
                    },
                    limits,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        ic_wire::decode(
            &reply,
            ic_wire::Encoding::Frame {
                method: id,
                direction: ic_wire::Direction::Response,
            },
            limits,
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

fn frame<T: Serialize>(method: u64, input: &T) -> Vec<u8> {
    ic_wire::encode(
        input,
        ic_wire::Encoding::Frame {
            method,
            direction: ic_wire::Direction::Request,
        },
        store::LIMITS,
    )
    .unwrap()
}

fn response<T: DeserializeOwned>(method: u64, bytes: &[u8]) -> T {
    ic_wire::decode(
        bytes,
        ic_wire::Encoding::Frame {
            method,
            direction: ic_wire::Direction::Response,
        },
        store::LIMITS,
    )
    .unwrap()
}
