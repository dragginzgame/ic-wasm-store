use super::{CanisterId, Client, store, support};

#[tokio::test(flavor = "current_thread")]
async fn removal_at_catalog_capacity_preserves_shared_bytes_and_pins() {
    let wasm = support::wasm();
    let owner = support::instance().await;
    let pic = &owner;
    let operator = CanisterId::self_authenticating(b"capacity controller");
    let id = pic
        .create_canister_with_settings(Some(operator), None)
        .await;
    pic.install_canister(id, wasm, vec![], Some(operator)).await;
    let client = Client { pic, id, operator };
    let quota = store::Quota {
        releases: 4096,
        reserved_bytes: 256 * 1024 * 1024,
    };
    let result: Result<store::Info, store::Error> = client
        .update("configure", store::CONFIGURE, &quota, store::LIMITS)
        .await;
    result.unwrap();
    let bytes = b"\0asm\x01\0\0\0";
    let pinned = store::Manifest {
        release: store::Release {
            name: "capacity".into(),
            version: "pinned".into(),
        },
        hash: store::digest(bytes),
        bytes: 8,
        chunks: vec![store::digest(bytes)],
    };
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &pinned, store::LIMITS)
        .await;
    result.unwrap();
    let result: Result<store::Status, store::Error> = client
        .update(
            "put",
            store::PUT,
            &store::Put {
                release: pinned.release.clone(),
                index: 0,
                bytes: bytes.to_vec(),
            },
            store::CHUNK_LIMITS,
        )
        .await;
    result.unwrap();
    let result: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &pinned.release, store::LIMITS)
        .await;
    result.unwrap();
    for operation in 1..=store::MAX_PINS {
        let result: Result<store::PinReceipt, store::Error> = client
            .update(
                "pin",
                store::PIN,
                &store::Pin {
                    operation: [operation as u8; 32],
                    manifest: pinned.clone(),
                },
                store::LIMITS,
            )
            .await;
        result.unwrap();
    }
    // A distinct, unpinned publication proves successful deletion at capacity,
    // in addition to the pinned refusal and incomplete-upload cancellation.
    let removable_bytes = b"\0asm\x01\0\0\0\0\x01\0";
    let removable = store::Manifest {
        release: store::Release {
            name: "capacity".into(),
            version: "removable".into(),
        },
        hash: store::digest(removable_bytes),
        bytes: removable_bytes.len() as u64,
        chunks: vec![store::digest(removable_bytes)],
    };
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &removable, store::LIMITS)
        .await;
    result.unwrap();
    let result: Result<store::Status, store::Error> = client
        .update(
            "put",
            store::PUT,
            &store::Put {
                release: removable.release.clone(),
                index: 0,
                bytes: removable_bytes.to_vec(),
            },
            store::CHUNK_LIMITS,
        )
        .await;
    result.unwrap();
    let result: Result<store::Status, store::Error> = client
        .update("publish", store::PUBLISH, &removable.release, store::LIMITS)
        .await;
    assert!(result.unwrap().published);
    let mut last = pinned.clone();
    let mut largest = pinned.clone();
    for index in 1u32..4095 {
        let hash = store::digest(&index.to_le_bytes());
        // 31 maximum-size reservations plus small artifacts fill the label ceiling
        // while staying within the real payload quota. Payloads remain resumable.
        let large = index <= 31;
        last = store::Manifest {
            release: store::Release {
                name: "capacity".into(),
                version: format!("{index:04}"),
            },
            hash,
            bytes: if large {
                (store::CHUNK_BYTES * store::MAX_CHUNKS) as u64
            } else {
                8
            },
            chunks: if large {
                (0..store::MAX_CHUNKS)
                    .map(|chunk| store::digest(format!("{index}:{chunk}").as_bytes()))
                    .collect()
            } else {
                vec![pinned.hash]
            },
        };
        let result: Result<store::Status, store::Error> = client
            .update("prepare", store::PREPARE, &last, store::LIMITS)
            .await;
        result.unwrap();
        if large {
            largest = last.clone();
        }
    }
    assert_eq!(client.info().await.releases, 4096);
    let extra = store::Manifest {
        release: store::Release {
            version: "overflow".into(),
            ..last.release.clone()
        },
        ..last.clone()
    };
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &extra, store::LIMITS)
        .await;
    assert_eq!(result, Err(store::Error::Capacity));
    let before_delete = client.info().await;
    for _ in 0..2 {
        let result: Result<store::Manifest, store::Error> = client
            .update(
                "remove",
                store::REMOVE,
                &store::Remove::Delete(removable.clone()),
                store::LIMITS,
            )
            .await;
        assert_eq!(result.unwrap(), removable);
    }
    let after_delete = client.info().await;
    assert_eq!(
        after_delete.reserved_bytes,
        before_delete.reserved_bytes - removable.bytes
    );
    assert_eq!(after_delete.releases, 4096);
    assert_eq!(after_delete.chunks, before_delete.chunks - 1);
    // A tombstone still consumes its permanent label even after reclaiming bytes.
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &extra, store::LIMITS)
        .await;
    assert_eq!(result, Err(store::Error::Capacity));
    let before = client.info().await;
    let started = std::time::Instant::now();
    let result: Result<store::Manifest, store::Error> = client
        .update(
            "remove",
            store::REMOVE,
            &store::Remove::Cancel(last.clone()),
            store::LIMITS,
        )
        .await;
    result.unwrap();
    println!(
        "capacity cancellation: 4096 retained labels, 32 retained pins, elapsed {:?}",
        started.elapsed()
    );
    let after = client.info().await;
    assert_eq!(after.reserved_bytes, before.reserved_bytes - 8);
    assert_eq!(after.releases, 4096);
    assert_eq!(after.chunks, 1);
    let result: Result<store::Manifest, store::Error> = client
        .update(
            "remove",
            store::REMOVE,
            &store::Remove::Delete(pinned.clone()),
            store::LIMITS,
        )
        .await;
    assert_eq!(result, Err(store::Error::Conflict));
    assert!(client.status(&pinned.release).await.published);
    let result: Result<store::Status, store::Error> = client
        .update("prepare", store::PREPARE, &last, store::LIMITS)
        .await;
    assert!(result.unwrap().missing.is_empty());
    assert_eq!(client.info().await.reserved_bytes, before.reserved_bytes);
    let result: Result<store::Manifest, store::Error> = client
        .update(
            "remove",
            store::REMOVE,
            &store::Remove::Cancel(largest.clone()),
            store::LIMITS,
        )
        .await;
    assert_eq!(result.unwrap(), largest);
    let after = client.info().await;
    assert_eq!(after.reserved_bytes, before.reserved_bytes - largest.bytes);
    assert_eq!(after.releases, 4096);
    assert_eq!(after.chunks, 1);
    owner.drop().await;
}
