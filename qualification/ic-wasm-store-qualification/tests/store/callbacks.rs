use super::{CanisterId, Client, frame, response, store, support};

// Sweep a bounded set of round boundaries and require a witness where the single
// missing chunk was actually uploaded. Authority/epoch refusal after that effect
// can only be the final upload continuation, not the initial observation reply.
#[tokio::test(flavor = "current_thread")]
async fn final_cache_callback_rechecks_authority_and_reopened_epoch() {
    let wasm = support::wasm();
    let owner = support::instance().await;
    let pic = &owner;
    let operator = CanisterId::self_authenticating(b"callback controller");
    let replacement = CanisterId::self_authenticating(b"callback replacement");
    for revoke in [true, false] {
        let mut witnessed = false;
        for schedule in 0..48 {
            let rounds = schedule / 4 + 1;
            let id = pic
                .create_canister_with_settings(Some(operator), None)
                .await;
            pic.install_canister(id, wasm.clone(), vec![], Some(operator))
                .await;
            let client = Client { pic, id, operator };
            let bytes = b"\0asm\x01\0\0\0";
            let manifest = store::Manifest {
                release: store::Release {
                    name: "callback".into(),
                    version: "one".into(),
                },
                hash: store::digest(bytes),
                bytes: 8,
                chunks: vec![store::digest(bytes)],
            };
            let result: Result<store::Status, store::Error> = client
                .update("prepare", store::PREPARE, &manifest, store::LIMITS)
                .await;
            result.unwrap();
            let result: Result<store::Status, store::Error> = client
                .update(
                    "put",
                    store::PUT,
                    &store::Put {
                        release: manifest.release.clone(),
                        index: 0,
                        bytes: bytes.to_vec(),
                    },
                    store::CHUNK_LIMITS,
                )
                .await;
            result.unwrap();
            let result: Result<store::Status, store::Error> = client
                .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
                .await;
            result.unwrap();
            let pin = store::Pin {
                operation: [1; 32],
                manifest: manifest.clone(),
            };
            let result: Result<store::PinReceipt, store::Error> =
                client.update("pin", store::PIN, &pin, store::LIMITS).await;
            result.unwrap();
            let request = store::PrepareCache {
                pin,
                expires_at_ns: pic.get_time().await.as_nanos_since_unix_epoch() + 300_000_000_000,
            };
            let pending = pic
                .submit_call(
                    id,
                    operator,
                    "prepare_cache",
                    frame(store::PREPARE_CACHE, &request),
                )
                .await
                .unwrap();
            for _ in 0..rounds {
                pic.tick().await;
            }
            if pic.ingress_status(pending.clone()).await.is_some() {
                break;
            }
            if revoke {
                pic.set_controllers(id, Some(operator), vec![replacement])
                    .await
                    .unwrap();
            } else {
                let attempt = store::MaintenanceAttempt {
                    sequence: 1,
                    operation: [2; 32],
                };
                // Queue both commands before driving rounds: no await_call can
                // accidentally finish the cache continuation between submissions.
                let close = pic
                    .submit_call(
                        id,
                        operator,
                        "admission",
                        frame(
                            store::ADMISSION,
                            &store::AdmissionCommand::Close(attempt.clone()),
                        ),
                    )
                    .await
                    .unwrap();
                let open = pic
                    .submit_call(
                        id,
                        operator,
                        "admission",
                        frame(
                            store::ADMISSION,
                            &store::AdmissionCommand::Activate(attempt),
                        ),
                    )
                    .await
                    .unwrap();
                let closed: Result<Option<store::Admission>, store::Error> =
                    response(store::ADMISSION, &pic.await_call(close).await.unwrap());
                let opened: Result<Option<store::Admission>, store::Error> =
                    response(store::ADMISSION, &pic.await_call(open).await.unwrap());
                assert!(closed.unwrap().unwrap().closed);
                if opened == Err(store::Error::Conflict) {
                    pic.await_call(pending).await.unwrap();
                    continue;
                }
                assert!(!opened.unwrap().unwrap().closed);
            }
            let result: Result<store::CacheReady, store::CacheError> = response(
                store::PREPARE_CACHE,
                &pic.await_call(pending).await.unwrap(),
            );
            let hashes = pic
                .stored_chunks(id, Some(if revoke { replacement } else { operator }))
                .await
                .unwrap();
            let expected = if revoke {
                store::CacheError::Authority
            } else {
                store::CacheError::Pin(store::Error::Conflict)
            };
            println!(
                "schedule: revoke={revoke}, rounds={rounds}, result={result:?}, chunks={}",
                hashes.len()
            );
            if result == Err(expected) && hashes == vec![manifest.hash.to_vec()] {
                println!(
                    "final callback witness: revoke={revoke}, rounds={rounds}, uploaded chunk retained"
                );
                witnessed = true;
                break;
            }
        }
        assert!(witnessed, "no final callback witness for revoke={revoke}");
    }
    owner.drop().await;
}
