//! Replicated delivery exercises Store's actual caller and maintenance gates.

use super::{CanisterId, Client, frame, store, support};
use std::time::Duration;

#[tokio::test(flavor = "current_thread")]
async fn pinned_delivery_is_scoped_replayable_and_fenced() {
    let pic = support::instance().await;
    let owner = CanisterId::self_authenticating(b"Store controller");
    let installer = CanisterId::self_authenticating(b"named installer");
    let sibling = CanisterId::self_authenticating(b"other installer");
    let id = pic.create_canister_with_settings(Some(owner), None).await;
    pic.install_canister(id, support::wasm(), vec![], Some(owner))
        .await;
    let controller = Client {
        pic: &pic,
        id,
        operator: owner,
    };
    let installer_client = Client {
        operator: installer,
        ..controller
    };
    let sibling_client = Client {
        operator: sibling,
        ..controller
    };

    let mut bytes = vec![42; store::CHUNK_BYTES];
    bytes[..8].copy_from_slice(b"\0asm\x01\0\0\0");
    let digest = store::digest(&bytes);
    let release = store::Release {
        name: "child".into(),
        version: "v1".into(),
    };
    let manifest = store::Manifest {
        release: release.clone(),
        hash: digest,
        bytes: bytes.len() as u64,
        chunks: vec![digest],
    };
    let prepared: Result<store::Status, store::Error> = controller
        .update("prepare", store::PREPARE, &manifest, store::LIMITS)
        .await;
    prepared.unwrap();
    let uploaded: Result<store::Status, store::Error> = controller
        .update(
            "put",
            store::PUT,
            &store::Put {
                release: release.clone(),
                index: 0,
                bytes: bytes.clone(),
            },
            store::CHUNK_LIMITS,
        )
        .await;
    uploaded.unwrap();
    let published: Result<store::Status, store::Error> = controller
        .update("publish", store::PUBLISH, &release, store::LIMITS)
        .await;
    assert!(published.unwrap().published);
    let mut alias = manifest.clone();
    alias.release.version = "alias".into();
    let prepared: Result<store::Status, store::Error> = controller
        .update("prepare", store::PREPARE, &alias, store::LIMITS)
        .await;
    assert!(prepared.unwrap().published);
    let pin = store::Pin {
        operation: [1; 32],
        manifest,
    };
    let retained: Result<store::PinReceipt, store::Error> = controller
        .update("pin", store::PIN, &pin, store::LIMITS)
        .await;
    assert_eq!(retained.unwrap().pin, pin);

    let now = pic.get_time().await.as_nanos_since_unix_epoch();
    let mut grant = store::ReadGrant {
        pin,
        installer: installer.as_slice().to_vec(),
        epoch: 0,
        expires_at_ns: now + 60_000_000_000,
    };
    assert!(
        pic.update_call(
            id,
            installer,
            "grant_read",
            frame(store::GRANT_READ, &grant)
        )
        .await
        .is_err()
    );
    assert!(
        pic.query_call(
            id,
            installer,
            "read",
            frame(
                store::READ,
                &store::Read {
                    release: release.clone(),
                    index: 0
                }
            )
        )
        .await
        .is_err()
    );

    for _ in 0..2 {
        let issued: Result<(), store::Error> = controller
            .update("grant_read", store::GRANT_READ, &grant, store::LIMITS)
            .await;
        assert_eq!(issued, Ok(()));
    }
    let mut request = store::GrantedRead {
        owner: owner.as_slice().to_vec(),
        grant: grant.clone(),
        index: 0,
    };
    assert_read_ingress_refused(&sibling_client, &request).await;
    request.grant.pin.manifest = alias;
    assert_eq!(
        read(&installer_client, &request).await,
        Err(store::Error::NotFound)
    );
    request.grant = grant.clone();
    request.grant.pin.manifest.hash[0] ^= 1;
    assert_eq!(
        read(&installer_client, &request).await,
        Err(store::Error::NotFound)
    );
    request.grant = grant.clone();
    request.index = 1;
    assert_eq!(
        read(&installer_client, &request).await,
        Err(store::Error::Invalid)
    );
    request.index = 0;
    assert_eq!(read(&installer_client, &request).await.unwrap().0, bytes);

    let candidate = std::fs::read(std::env::var_os("IC_WASM_STORE_CANDIDATE").unwrap()).unwrap();
    pic.stop_canister(id, Some(owner)).await.unwrap();
    pic.upgrade_canister(id, candidate, vec![], Some(owner))
        .await
        .unwrap();
    pic.start_canister(id, Some(owner)).await.unwrap();
    assert_eq!(read(&installer_client, &request).await.unwrap().0, bytes);

    pic.set_controllers(id, Some(owner), vec![sibling])
        .await
        .unwrap();
    assert_read_ingress_refused(&installer_client, &request).await;
    pic.set_controllers(id, Some(sibling), vec![owner])
        .await
        .unwrap();

    pic.advance_time(Duration::from_secs(60)).await;
    pic.tick().await;
    assert_read_ingress_refused(&installer_client, &request).await;

    grant.expires_at_ns = pic.get_time().await.as_nanos_since_unix_epoch() + 60_000_000_000;
    let issued: Result<(), store::Error> = controller
        .update("grant_read", store::GRANT_READ, &grant, store::LIMITS)
        .await;
    assert_eq!(issued, Ok(()));
    request.grant = grant;
    let attempt = store::MaintenanceAttempt {
        sequence: 1,
        operation: [2; 32],
    };
    let closed: Result<Option<store::Admission>, store::Error> = controller
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Close(attempt.clone()),
            store::LIMITS,
        )
        .await;
    assert!(closed.unwrap().unwrap().closed);
    assert_read_ingress_refused(&installer_client, &request).await;
    let opened: Result<Option<store::Admission>, store::Error> = controller
        .update(
            "admission",
            store::ADMISSION,
            &store::AdmissionCommand::Activate(attempt),
            store::LIMITS,
        )
        .await;
    assert!(!opened.unwrap().unwrap().closed);
    assert_read_ingress_refused(&installer_client, &request).await;
    let stale: Result<(), store::Error> = controller
        .update(
            "grant_read",
            store::GRANT_READ,
            &request.grant,
            store::LIMITS,
        )
        .await;
    assert_eq!(stale, Err(store::Error::Conflict));
}

async fn read(
    client: &Client<'_>,
    input: &store::GrantedRead,
) -> Result<store::Chunk, store::Error> {
    client
        .update(
            "read_granted",
            store::READ_GRANTED,
            input,
            store::CHUNK_LIMITS,
        )
        .await
}

async fn assert_read_ingress_refused(client: &Client<'_>, request: &store::GrantedRead) {
    let refusal = client
        .pic
        .submit_call(
            client.id,
            client.operator,
            "read_granted",
            frame(store::READ_GRANTED, request),
        )
        .await
        .unwrap_err();
    assert_eq!(
        refusal.error_code,
        ic_testkit::pocket_ic::ErrorCode::CanisterRejectedMessage
    );
}
