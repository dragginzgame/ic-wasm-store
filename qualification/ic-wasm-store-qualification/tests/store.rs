//! Installed Store upload custody, raw chunk integrity and independent maintenance.
use ic_testkit::pocket_ic::{CanisterId, nonblocking::PocketIc};
use ic_wasm_store_protocol as store;
use serde::{Serialize, de::DeserializeOwned};
mod support;

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
    async fn info(&self) -> store::Info {
        let reply = self
            .pic
            .query_call(
                self.id,
                self.operator,
                "info",
                ic_wire::encode(
                    &(),
                    ic_wire::Encoding::Frame {
                        method: store::INFO,
                        direction: ic_wire::Direction::Request,
                    },
                    store::LIMITS,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        ic_wire::decode(
            &reply,
            ic_wire::Encoding::Frame {
                method: store::INFO,
                direction: ic_wire::Direction::Response,
            },
            store::LIMITS,
        )
        .unwrap()
    }
    async fn status(&self, release: &store::Release) -> store::Status {
        self.update::<_, Result<store::Status, store::Error>>(
            "observe",
            store::STATUS,
            release,
            store::LIMITS,
        )
        .await
        .unwrap()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn versioned_upload_resumes_across_two_executables_and_reuses_bytes() {
    let (original, binary) = support::inputs();
    let candidate = std::fs::read(
        std::env::var_os("IC_WASM_STORE_CANDIDATE").expect("run make qualification-test"),
    )
    .unwrap();
    assert_ne!(original, candidate);
    let owner = support::instance(&binary).await;
    let pic = owner.instance();
    let operator = CanisterId::self_authenticating(b"store qualification controller");
    let id = pic
        .create_canister_with_settings(Some(operator), None)
        .await;
    pic.install_canister(id, original, vec![], Some(operator))
        .await;
    let client = Client { pic, id, operator };
    // More than the old combined Root budget. Integrity publication is not Wasm validation.
    let mut bytes = vec![0; store::CHUNK_BYTES * store::MAX_CHUNKS];
    for (index, chunk) in bytes.chunks_mut(store::CHUNK_BYTES).enumerate() {
        chunk.fill(index as u8);
    }
    bytes[..8].copy_from_slice(b"\0asm\x01\0\0\0");
    let manifest = store::Manifest {
        release: store::Release {
            name: "app".into(),
            version: "1.0.0".into(),
        },
        hash: store::digest(&bytes),
        bytes: bytes.len() as u64,
        chunks: bytes
            .chunks(store::CHUNK_BYTES)
            .map(store::digest)
            .collect(),
    };
    assert!(
        pic.update_call(
            id,
            CanisterId::anonymous(),
            "prepare",
            ic_wire::encode(
                &manifest,
                ic_wire::Encoding::Frame {
                    method: store::PREPARE,
                    direction: ic_wire::Direction::Request
                },
                store::LIMITS
            )
            .unwrap()
        )
        .await
        .is_err()
    );
    assert!(
        pic.update_call(id, operator, "put", vec![0; store::CHUNK_LIMITS.bytes + 1])
            .await
            .is_err()
    );
    let initial = client.info().await;
    assert_eq!(initial.releases, 0);
    let prepared: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &manifest, store::LIMITS)
        .await;
    assert!(!prepared.unwrap().published);
    let first = store::Put {
        release: manifest.release.clone(),
        index: 0,
        bytes: bytes[..store::CHUNK_BYTES].to_vec(),
    };
    let _: Result<store::Status, store::Error> = client
        .update("put", store::PUT, &first, store::CHUNK_LIMITS)
        .await;
    let partial = client.status(&manifest.release).await;
    assert!(!partial.missing.contains(&0));
    let blocked: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
        .await;
    assert_eq!(blocked, Err(store::Error::Incomplete));
    pic.stop_canister(id, Some(operator)).await.unwrap();
    pic.upgrade_canister(id, candidate, vec![], Some(operator))
        .await
        .unwrap();
    pic.start_canister(id, Some(operator)).await.unwrap();
    assert!(client.info().await.maintenance_candidate);
    assert_eq!(client.status(&manifest.release).await, partial);
    // Resume only reported missing indices, at the complete admitted artifact bound.
    for index in partial.missing {
        let start = index as usize * store::CHUNK_BYTES;
        let end = (start + store::CHUNK_BYTES).min(bytes.len());
        let input = store::Put {
            release: manifest.release.clone(),
            index,
            bytes: bytes[start..end].to_vec(),
        };
        let result: Result<store::Status, store::Error> = client
            .update("put", store::PUT, &input, store::CHUNK_LIMITS)
            .await;
        result.unwrap();
    }
    let published: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
        .await;
    let published = published.unwrap();
    assert!(published.published);
    let repeated: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
        .await;
    assert_eq!(repeated.unwrap(), published);
    let usage = client.info().await;
    assert_eq!(usage.chunks, store::MAX_CHUNKS as u64);
    let second = store::Manifest {
        release: store::Release {
            version: "2.0.0".into(),
            ..manifest.release.clone()
        },
        ..manifest.clone()
    };
    let reused: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &second, store::LIMITS)
        .await;
    assert!(reused.unwrap().published);
    assert_eq!(client.info().await.chunks, usage.chunks);
    assert_eq!(client.info().await.reserved_bytes, usage.reserved_bytes);
    let mut conflict = manifest.clone();
    conflict.hash[0] ^= 1;
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &conflict, store::LIMITS)
        .await;
    assert_eq!(result, Err(store::Error::Conflict));
    let too_small = store::Quota {
        reserved_bytes: 1,
        releases: 1,
    };
    let result: Result<store::Info, store::Error> = client
        .update("configure", store::CONFIGURE, &too_small, store::LIMITS)
        .await;
    assert_eq!(result, Err(store::Error::Capacity));
    let changed_quota = store::Quota {
        reserved_bytes: usage.reserved_bytes * 2,
        releases: 10,
    };
    let result: Result<store::Info, store::Error> = client
        .update("configure", store::CONFIGURE, &changed_quota, store::LIMITS)
        .await;
    assert_eq!(result.unwrap().quota, changed_quota);
    // Replicated reads recover exact bytes without relying on uncertified query results.
    let mut recovered = Vec::new();
    for index in 0..manifest.chunks.len() {
        let input = store::Read {
            release: manifest.release.clone(),
            index: index as u32,
        };
        let reply = pic
            .update_call(
                id,
                operator,
                "read_chunk",
                ic_wire::encode(
                    &input,
                    ic_wire::Encoding::Frame {
                        method: store::READ,
                        direction: ic_wire::Direction::Request,
                    },
                    store::LIMITS,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let chunk: Result<store::Chunk, store::Error> = ic_wire::decode(
            &reply,
            ic_wire::Encoding::Frame {
                method: store::READ,
                direction: ic_wire::Direction::Response,
            },
            store::CHUNK_LIMITS,
        )
        .unwrap();
        recovered.extend(chunk.unwrap().0);
    }
    assert_eq!(recovered, bytes);
    let reply = pic
        .query_call(
            id,
            operator,
            "list",
            ic_wire::encode(
                &Option::<store::Release>::None,
                ic_wire::Encoding::Frame {
                    method: store::LIST,
                    direction: ic_wire::Direction::Request,
                },
                store::LIMITS,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let listed: Result<Vec<store::Status>, store::Error> = ic_wire::decode(
        &reply,
        ic_wire::Encoding::Frame {
            method: store::LIST,
            direction: ic_wire::Direction::Response,
        },
        store::LIMITS,
    )
    .unwrap();
    assert_eq!(listed.unwrap().len(), 2);
    // Remove aliases independently, reclaim the last active payload, and reopen tombstones.
    for release in [&second, &manifest] {
        let removed: Result<store::Manifest, store::Error> = client
            .update("delete", store::DELETE, release, store::LIMITS)
            .await;
        assert_eq!(removed.unwrap(), *release);
    }
    assert_eq!(client.info().await.reserved_bytes, 0);
    assert_eq!(client.info().await.chunks, 0);
    let candidate = std::fs::read(std::env::var_os("IC_WASM_STORE_CANDIDATE").unwrap()).unwrap();
    pic.upgrade_canister(id, candidate, vec![], Some(operator))
        .await
        .unwrap();
    let removed: Result<Vec<store::Manifest>, store::Error> = client
        .update(
            "tombstones",
            store::TOMBSTONES,
            &Option::<store::Release>::None,
            store::LIMITS,
        )
        .await;
    assert_eq!(removed.unwrap(), vec![manifest.clone(), second.clone()]);
    let conflict: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &conflict, store::LIMITS)
        .await;
    assert_eq!(conflict, Err(store::Error::Conflict));
    let restored: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &manifest, store::LIMITS)
        .await;
    assert_eq!(restored.unwrap().missing.len(), store::MAX_CHUNKS);
    let cancelled: Result<store::Manifest, store::Error> = client
        .update("cancel", store::CANCEL, &manifest, store::LIMITS)
        .await;
    assert_eq!(cancelled.unwrap(), manifest);
    assert_eq!(client.info().await.reserved_bytes, 0);
    let restored: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &manifest, store::LIMITS)
        .await;
    for index in restored.unwrap().missing {
        let start = index as usize * store::CHUNK_BYTES;
        let input = store::Put {
            release: manifest.release.clone(),
            index,
            bytes: bytes[start..start + store::CHUNK_BYTES].to_vec(),
        };
        let result: Result<store::Status, store::Error> = client
            .update("put", store::PUT, &input, store::CHUNK_LIMITS)
            .await;
        result.unwrap();
    }
    let result: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
        .await;
    assert!(result.unwrap().published);
    let pin = store::Pin {
        operation: [1; 32],
        manifest: manifest.clone(),
    };
    let receipt: Result<store::PinReceipt, store::Error> =
        client.update("pin", store::PIN, &pin, store::LIMITS).await;
    assert_eq!(receipt.unwrap().pin, pin);
    let protected: Result<store::Manifest, store::Error> = client
        .update("delete", store::DELETE, &manifest, store::LIMITS)
        .await;
    assert_eq!(protected, Err(store::Error::Conflict));
    let request = store::PrepareCache {
        pin: pin.clone(),
        expires_at_ns: pic.get_time().await.as_nanos_since_unix_epoch() + 300_000_000_000,
    };
    let ready: Result<store::CacheReady, store::CacheError> = client
        .update(
            "prepare_cache",
            store::PREPARE_CACHE,
            &request,
            store::LIMITS,
        )
        .await;
    assert_eq!(ready.unwrap().uploaded, store::MAX_CHUNKS as u32);
    let ready: Result<store::CacheReady, store::CacheError> = client
        .update(
            "prepare_cache",
            store::PREPARE_CACHE,
            &request,
            store::LIMITS,
        )
        .await;
    assert_eq!(ready.unwrap().uploaded, 0);
    let attempt = store::MaintenanceAttempt {
        sequence: 1,
        operation: [2; 32],
    };
    let closed: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Close(attempt.clone()),
            store::LIMITS,
        )
        .await;
    assert!(closed.unwrap().unwrap().closed);
    let blocked: Result<store::CacheReady, store::CacheError> = client
        .update(
            "prepare_cache",
            store::PREPARE_CACHE,
            &request,
            store::LIMITS,
        )
        .await;
    assert_eq!(blocked, Err(store::CacheError::Pin(store::Error::Closed)));
    let reopened: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Activate(attempt.clone()),
            store::LIMITS,
        )
        .await;
    assert!(!reopened.unwrap().unwrap().closed);
    let replay: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Close(attempt),
            store::LIMITS,
        )
        .await;
    assert!(!replay.unwrap().unwrap().closed);
    let ready: Result<store::CacheReady, store::CacheError> = client
        .update(
            "prepare_cache",
            store::PREPARE_CACHE,
            &request,
            store::LIMITS,
        )
        .await;
    assert_eq!(ready.unwrap().pin, pin);
    // Submit without driving the call to completion, then interleave closure.
    pic.clear_chunk_store(id, Some(operator)).await.unwrap();
    let submit = || {
        ic_wire::encode(
            &request,
            ic_wire::Encoding::Frame {
                method: store::PREPARE_CACHE,
                direction: ic_wire::Direction::Request,
            },
            store::LIMITS,
        )
        .unwrap()
    };
    let pending = pic
        .submit_call(id, operator, "prepare_cache", submit())
        .await
        .unwrap();
    pic.tick().await;
    assert!(pic.ingress_status(pending.clone()).await.is_none());
    let attempt = store::MaintenanceAttempt {
        sequence: 2,
        operation: [3; 32],
    };
    let closed: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Close(attempt.clone()),
            store::LIMITS,
        )
        .await;
    assert!(closed.unwrap().unwrap().closed);
    let reply = pic.await_call(pending).await.unwrap();
    let result: Result<store::CacheReady, store::CacheError> = ic_wire::decode(
        &reply,
        ic_wire::Encoding::Frame {
            method: store::PREPARE_CACHE,
            direction: ic_wire::Direction::Response,
        },
        store::LIMITS,
    )
    .unwrap();
    assert_eq!(result, Err(store::CacheError::Pin(store::Error::Closed)));
    let reopened: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Activate(attempt),
            store::LIMITS,
        )
        .await;
    assert!(!reopened.unwrap().unwrap().closed);

    // Revoking the original caller must prevent readiness on an outstanding call.
    pic.clear_chunk_store(id, Some(operator)).await.unwrap();
    let pending = pic
        .submit_call(id, operator, "prepare_cache", submit())
        .await
        .unwrap();
    pic.tick().await;
    assert!(pic.ingress_status(pending.clone()).await.is_none());
    let replacement = CanisterId::self_authenticating(b"replacement controller");
    pic.set_controllers(id, Some(operator), vec![replacement])
        .await
        .unwrap();
    let reply = pic.await_call(pending).await.unwrap();
    let result: Result<store::CacheReady, store::CacheError> = ic_wire::decode(
        &reply,
        ic_wire::Encoding::Frame {
            method: store::PREPARE_CACHE,
            direction: ic_wire::Direction::Response,
        },
        store::LIMITS,
    )
    .unwrap();
    assert_eq!(result, Err(store::CacheError::Authority));
    println!(
        "Store: {} payload bytes, {} unique chunks, two retained versions; upload resumed after changed executable",
        bytes.len(),
        usage.chunks
    );
    owner.close().await;
}
