use super::{CanisterId, Client, frame, store, support};

#[tokio::test(flavor = "current_thread")]
async fn closed_upgrade_preserves_removal_fences_and_exact_activation_owner() {
    let wasm = support::wasm();
    let owner = support::instance().await;
    let pic = &owner;
    let operator = CanisterId::self_authenticating(b"closed upgrade owner");
    let other = CanisterId::self_authenticating(b"other authorized controller");
    let id = pic
        .create_canister_with_settings(Some(operator), None)
        .await;
    pic.install_canister(id, wasm, vec![], Some(operator)).await;
    pic.set_controllers(id, Some(operator), vec![operator, other])
        .await
        .unwrap();
    // Runtime's explicit refusal handler traps before decoding or state access.
    for (caller, bytes, diagnostic) in [
        (CanisterId::anonymous(), vec![], "Store controller required"),
        (
            operator,
            vec![0; store::LIMITS.bytes + 1],
            "Store frame too large",
        ),
    ] {
        let refusal = pic
            .update_call(id, caller, "observe_info", bytes)
            .await
            .unwrap_err();
        assert_eq!(refusal.reject_code as u64, 5);
        assert!(refusal.reject_message.contains(diagnostic));
    }
    let client = Client { pic, id, operator };
    let mut manifests = Vec::new();
    for index in 0..3 {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        bytes.extend(vec![0; index]);
        let manifest = store::Manifest {
            release: store::Release {
                name: "closed-upgrade".into(),
                version: index.to_string(),
            },
            hash: store::digest(&bytes),
            bytes: bytes.len() as u64,
            chunks: vec![store::digest(&bytes)],
        };
        let result: Result<store::Status, store::Error> = client
            .update("prepare", store::PREPARE, &manifest, store::LIMITS)
            .await;
        result.unwrap();
        if index == 0 {
            let result: Result<store::Status, store::Error> = client
                .update(
                    "put",
                    store::PUT,
                    &store::Put {
                        release: manifest.release.clone(),
                        index: 0,
                        bytes,
                    },
                    store::CHUNK_LIMITS,
                )
                .await;
            result.unwrap();
            let result: Result<store::Status, store::Error> = client
                .update("publish", store::PUBLISH, &manifest.release, store::LIMITS)
                .await;
            result.unwrap();
        } else if index == 2 {
            let result: Result<store::Manifest, store::Error> = client
                .update("cancel", store::CANCEL, &manifest, store::LIMITS)
                .await;
            result.unwrap();
        }
        manifests.push(manifest);
    }
    let before = client.info().await;
    let attempt = store::MaintenanceAttempt {
        sequence: 1,
        operation: [6; 32],
    };
    // Exercise the installed guards for each removal endpoint. The valid
    // controller calls below distinguish rejected input from a broken endpoint.
    for (method, request) in [
        ("delete", frame(store::DELETE, &manifests[0])),
        ("cancel", frame(store::CANCEL, &manifests[1])),
        (
            "tombstones",
            frame(store::TOMBSTONES, &Option::<store::Release>::None),
        ),
    ] {
        assert!(
            pic.update_call(id, CanisterId::anonymous(), method, request)
                .await
                .is_err()
        );
        assert!(
            pic.update_call(id, operator, method, vec![0; store::LIMITS.bytes + 1])
                .await
                .is_err()
        );
    }
    for (method, correct, incorrect, manifest) in [
        ("delete", store::DELETE, store::CANCEL, &manifests[0]),
        ("cancel", store::CANCEL, store::DELETE, &manifests[1]),
    ] {
        assert!(
            pic.update_call(id, operator, method, frame(incorrect, manifest))
                .await
                .is_err()
        );
        let reply_frame = ic_wire::encode(
            manifest,
            ic_wire::Encoding::Frame {
                method: correct,
                direction: ic_wire::Direction::Response,
            },
            store::LIMITS,
        )
        .unwrap();
        assert!(
            pic.update_call(id, operator, method, reply_frame)
                .await
                .is_err()
        );
        let mut conflicting = manifest.clone();
        conflicting.hash[0] ^= 1;
        let result: Result<store::Manifest, store::Error> = client
            .update(method, correct, &conflicting, store::LIMITS)
            .await;
        assert_eq!(result, Err(store::Error::Conflict));
    }
    assert_eq!(client.info().await, before);
    let closed: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Close(attempt.clone()),
            store::LIMITS,
        )
        .await;
    let closed = closed.unwrap();
    let preserved = preservation_pages(&client, &attempt).await;
    assert_eq!(preserved.len(), 4); // Three labels and explicit end, not active count.
    assert_eq!(
        preserved[2].entry,
        Some(store::preservation::Entry::Removed(manifests[2].clone()))
    );
    let request = store::preservation::Request {
        attempt: attempt.clone(),
        after: None,
    };
    for caller in [CanisterId::anonymous(), other] {
        let reply = pic
            .update_call(
                id,
                caller,
                "observe_preservation",
                frame(store::PRESERVATION, &request),
            )
            .await;

        if caller == other {
            let decoded: Result<store::preservation::Page, store::Error> = ic_wire::decode(
                &reply.unwrap(),
                ic_wire::Encoding::Frame {
                    method: store::PRESERVATION,
                    direction: ic_wire::Direction::Response,
                },
                store::LIMITS,
            )
            .unwrap();
            assert_eq!(decoded, Err(store::Error::Conflict));
        } else {
            let refusal = reply.unwrap_err();
            assert_eq!(refusal.reject_code as u64, 5);
            assert!(refusal.reject_message.contains("Store controller required"));
        }
    }
    for (malformed, diagnostic) in [
        (frame(113, &request), "Decode(Identity)"),
        (vec![0; store::LIMITS.bytes + 1], "Store frame too large"),
    ] {
        let refusal = pic
            .update_call(id, operator, "observe_preservation", malformed)
            .await
            .unwrap_err();
        assert_eq!(refusal.reject_code as u64, 5);
        assert!(
            refusal.reject_message.contains(diagnostic),
            "expected {diagnostic} in {refusal:?}"
        );
    }
    assert_eq!(preservation_pages(&client, &attempt).await, preserved);
    let candidate = std::fs::read(std::env::var_os("IC_WASM_STORE_CANDIDATE").unwrap()).unwrap();
    pic.stop_canister(id, Some(operator)).await.unwrap();
    pic.upgrade_canister(id, candidate, vec![], Some(operator))
        .await
        .unwrap();
    pic.start_canister(id, Some(operator)).await.unwrap();
    let observed: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Observe,
            store::LIMITS,
        )
        .await;
    assert_eq!(observed.unwrap(), closed);
    assert_eq!(preservation_pages(&client, &attempt).await, preserved);
    for (method, number, manifest) in [
        ("delete", store::DELETE, &manifests[0]),
        ("cancel", store::CANCEL, &manifests[1]),
        // Even an otherwise idempotent tombstone replay is fenced while closed.
        ("cancel", store::CANCEL, &manifests[2]),
    ] {
        let result: Result<store::Manifest, store::Error> =
            client.update(method, number, manifest, store::LIMITS).await;
        assert_eq!(result, Err(store::Error::Closed));
    }
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &manifests[2], store::LIMITS)
        .await;
    assert_eq!(result, Err(store::Error::Closed));
    assert!(!before.maintenance_candidate);
    assert_eq!(
        client.info().await,
        store::Info {
            maintenance_candidate: true,
            ..before
        }
    );
    assert!(client.status(&manifests[0].release).await.published);
    assert_eq!(client.status(&manifests[1].release).await.missing, vec![0]);
    let removed: Result<Vec<store::Manifest>, store::Error> = client
        .update(
            "tombstones",
            store::TOMBSTONES,
            &Option::<store::Release>::None,
            store::LIMITS,
        )
        .await;
    assert_eq!(removed.unwrap(), vec![manifests[2].clone()]);
    let alternate = Client {
        pic,
        id,
        operator: other,
    };
    let result: Result<Option<store::Admission>, store::Error> = alternate
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Activate(attempt.clone()),
            store::LIMITS,
        )
        .await;
    assert_eq!(result, Err(store::Error::Conflict));
    let observed: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Observe,
            store::LIMITS,
        )
        .await;
    assert_eq!(observed.unwrap(), closed);
    let active: Result<Option<store::Admission>, store::Error> = client
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Activate(attempt),
            store::LIMITS,
        )
        .await;
    assert!(!active.unwrap().unwrap().closed);
    let deleted: Result<store::Manifest, store::Error> = client
        .update("delete", store::DELETE, &manifests[0], store::LIMITS)
        .await;
    assert_eq!(deleted.unwrap(), manifests[0]);
    let cancelled: Result<store::Manifest, store::Error> = client
        .update("cancel", store::CANCEL, &manifests[1], store::LIMITS)
        .await;
    assert_eq!(cancelled.unwrap(), manifests[1]);
    let restored: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &manifests[2], store::LIMITS)
        .await;
    assert_eq!(restored.unwrap().missing, vec![0]);
    assert_eq!(client.info().await.reserved_bytes, manifests[2].bytes);
    owner.drop().await;
}

async fn preservation_pages(
    client: &Client<'_>,
    attempt: &store::MaintenanceAttempt,
) -> Vec<store::preservation::Page> {
    let mut request = store::preservation::Request {
        attempt: attempt.clone(),
        after: None,
    };
    let mut pages: Vec<store::preservation::Page> = Vec::new();
    for _ in 0..4 {
        let page = client
            .update::<_, Result<store::preservation::Page, store::Error>>(
                "observe_preservation",
                store::PRESERVATION,
                &request,
                store::LIMITS,
            )
            .await
            .unwrap();
        if let Some(previous) = pages.last() {
            assert_eq!(page.catalog, previous.catalog);
            if let Some(entry) = &page.entry {
                assert!(previous.entry.as_ref().unwrap().order_key() < entry.order_key());
            }
        }
        request.after = page
            .entry
            .as_ref()
            .map(|entry| entry.manifest().release.clone());
        pages.push(page);
        if request.after.is_none() {
            return pages;
        }
    }
    panic!("missing preservation end page");
}
