use super::*;
use ic_storage::VectorMemory;
use ic_wasm_store_protocol::CHUNK_BYTES;

fn fixture(version: &str, bytes: usize) -> (Manifest, Vec<u8>) {
    let mut payload = vec![42; bytes];
    payload[..8].copy_from_slice(b"\0asm\x01\0\0\0");
    (
        Manifest {
            release: Release {
                name: "app".into(),
                version: version.into(),
            },
            hash: digest(&payload),
            bytes: bytes as u64,
            chunks: payload.chunks(CHUNK_BYTES).map(digest).collect(),
        },
        payload,
    )
}
fn upload(state: &mut State<VectorMemory>, manifest: &Manifest, payload: &[u8]) {
    for (index, bytes) in payload.chunks(CHUNK_BYTES).enumerate() {
        state
            .put(&Put {
                release: manifest.release.clone(),
                index: index as u32,
                bytes: bytes.to_vec(),
            })
            .unwrap();
    }
}

#[test]
fn preservation_pages_require_one_closed_owner_and_keep_aliases_and_partial_uploads() {
    use ic_wasm_store_protocol::{AdmissionCommand, MaintenanceAttempt, preservation::Request};
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (first, bytes) = fixture("one", 33);
    state.prepare(&first).unwrap();
    upload(&mut state, &first, &bytes);
    state.publish(&first.release).unwrap();
    let alias = Manifest {
        release: Release {
            version: "two".into(),
            ..first.release.clone()
        },
        ..first.clone()
    };
    state.prepare(&alias).unwrap();
    state
        .pin(
            &[2],
            &Pin {
                operation: [2; 32],
                manifest: alias.clone(),
            },
        )
        .unwrap();
    let (pending, _) = fixture("unfinished", 41);
    state.prepare(&pending).unwrap();
    let attempt = MaintenanceAttempt {
        sequence: 1,
        operation: [1; 32],
    };
    let mut request = Request {
        attempt: attempt.clone(),
        after: None,
    };
    assert_eq!(state.preservation(&[1], &request), Err(Error::Conflict));
    state
        .admission(&[1], &AdmissionCommand::Close(attempt.clone()))
        .unwrap();
    assert_eq!(state.preservation(&[2], &request), Err(Error::Conflict));
    let first_page = state.preservation(&[1], &request).unwrap();
    assert_eq!(first_page.catalog.releases, 3);
    assert_eq!(first_page.catalog.artifacts, 2);
    assert_eq!(first_page.entry.as_ref().unwrap().pins[0].owner, vec![2]);
    drop(state);
    let mut state = State::open(backing, false);
    assert_eq!(state.preservation(&[1], &request).unwrap(), first_page);
    let mut releases = Vec::new();
    loop {
        let page = state.preservation(&[1], &request).unwrap();
        assert_eq!(page.catalog, first_page.catalog);
        let Some(entry) = page.entry else {
            break;
        };
        if entry.status.manifest == pending {
            assert_eq!(entry.status.missing, vec![0]);
        }
        request.after = Some(entry.status.manifest.release.clone());
        releases.push(entry.status.manifest.release);
    }
    assert_eq!(
        releases,
        vec![first.release, alias.release, pending.release]
    );
    request.after = Some(Release {
        name: String::new(),
        version: "bad".into(),
    });
    assert_eq!(state.preservation(&[1], &request), Err(Error::Invalid));
    request.after = None;
    state
        .admission(&[1], &AdmissionCommand::Activate(attempt))
        .unwrap();
    assert_eq!(state.preservation(&[1], &request), Err(Error::Conflict));
    state
        .admission(
            &[1],
            &AdmissionCommand::Close(MaintenanceAttempt {
                sequence: 2,
                operation: [2; 32],
            }),
        )
        .unwrap();
    assert_eq!(state.preservation(&[1], &request), Err(Error::Conflict));
}

#[test]
fn custody_reports_staging_and_all_shared_artifact_pins_after_reopen() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (first, bytes) = fixture("one", 33);
    assert_eq!(state.custody(&first.release), Err(Error::NotFound));
    state.prepare(&first).unwrap();
    let staged = state.custody(&first.release).unwrap();
    assert_eq!(staged.status.missing, vec![0]);
    assert!(!staged.status.published);
    assert!(staged.pins.is_empty());
    upload(&mut state, &first, &bytes);
    state.publish(&first.release).unwrap();
    let second = Manifest {
        release: Release {
            version: "two".into(),
            ..first.release.clone()
        },
        ..first.clone()
    };
    state.prepare(&second).unwrap();
    for (owner, manifest) in [(&[1][..], &first), (&[2][..], &second)] {
        state
            .pin(
                owner,
                &Pin {
                    operation: [7; 32],
                    manifest: manifest.clone(),
                },
            )
            .unwrap();
    }
    let expected = state.custody(&first.release).unwrap();
    assert_eq!(expected.pins.len(), 2);
    assert_eq!(expected.pins[0].owner, vec![1]);
    assert_eq!(expected.pins[1].owner, vec![2]);
    assert_eq!(expected.pins[0].release, first.release);
    assert_eq!(expected.pins[1].release, second.release);
    assert_eq!(state.custody(&second.release).unwrap().pins, expected.pins);
    drop(state);
    let state = State::open(backing, false);
    assert_eq!(state.custody(&first.release).unwrap(), expected);
    assert_eq!(
        state.custody(&Release {
            name: String::new(),
            version: "one".into()
        }),
        Err(Error::Invalid)
    );
    assert_eq!(state.custody(&first.release).unwrap(), expected);
}

#[test]
fn interrupted_upload_reopens_multiple_versions_without_duplicate_payloads() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (first, bytes) = fixture("1.0.0", 5 * CHUNK_BYTES + 31);
    state.prepare(&first).unwrap();
    state
        .put(&Put {
            release: first.release.clone(),
            index: 0,
            bytes: bytes[..CHUNK_BYTES].to_vec(),
        })
        .unwrap();
    let partial = state.status(&first.release).unwrap();
    assert!(!partial.published);
    assert_eq!(state.publish(&first.release), Err(Error::Incomplete));
    assert_eq!(
        state.read(&Read {
            release: first.release.clone(),
            index: 0
        }),
        Err(Error::Incomplete)
    );
    drop(state);
    let mut state = State::open(backing.clone(), false);
    assert_eq!(state.prepare(&first).unwrap(), partial);
    upload(&mut state, &first, &bytes);
    let published = state.publish(&first.release).unwrap();
    assert!(published.published);
    assert!(published.missing.is_empty());
    // Lost successful replies are handled by exact idempotent publication, not new identity.
    assert_eq!(state.publish(&first.release).unwrap(), published);
    let usage = state.info();
    let second = Manifest {
        release: Release {
            version: "2.0.0".into(),
            ..first.release.clone()
        },
        ..first.clone()
    };
    assert!(state.prepare(&second).unwrap().published);
    assert_eq!(state.info().reserved_bytes, usage.reserved_bytes);
    assert_eq!(state.info().chunks, usage.chunks);
    let (third, changed) = fixture("3.0.0", CHUNK_BYTES + 17);
    state.prepare(&third).unwrap();
    upload(&mut state, &third, &changed);
    state.publish(&third.release).unwrap();
    drop(state);
    let state = State::open(backing, false);
    assert_eq!(state.list(None).unwrap().len(), 3);
    assert_eq!(
        state
            .read(&Read {
                release: first.release,
                index: 5
            })
            .unwrap()
            .0,
        bytes[5 * CHUNK_BYTES..]
    );
}

#[test]
fn conflicts_hashes_and_quotas_refuse_without_changing_custody() {
    let mut state = State::open(VectorMemory::default(), true);
    let (manifest, bytes) = fixture("1", 37);
    state
        .configure(&Quota {
            reserved_bytes: 37,
            releases: 1,
        })
        .unwrap();
    state.prepare(&manifest).unwrap();
    let before = state.info();
    let mut conflict = manifest.clone();
    conflict.hash[0] ^= 1;
    assert_eq!(state.prepare(&conflict), Err(Error::Conflict));
    conflict = manifest.clone();
    conflict.chunks[0][0] ^= 1;
    assert_eq!(state.prepare(&conflict), Err(Error::Conflict));
    assert_eq!(
        state.put(&Put {
            release: manifest.release.clone(),
            index: 0,
            bytes: vec![7; 37]
        }),
        Err(Error::Hash)
    );
    assert_eq!(state.info(), before);
    let (second, _) = fixture("2", 38);
    assert_eq!(state.prepare(&second), Err(Error::Capacity));
    assert_eq!(
        state.configure(&Quota {
            reserved_bytes: 36,
            releases: 1
        }),
        Err(Error::Capacity)
    );
    upload(&mut state, &manifest, &bytes);
    state.publish(&manifest.release).unwrap();
    state
        .configure(&Quota {
            reserved_bytes: 75,
            releases: 2,
        })
        .unwrap();
    state.prepare(&second).unwrap();
    assert_eq!(state.info().reserved_bytes, 75);
    assert!(state.status(&manifest.release).unwrap().published);
}

#[test]
fn full_digest_and_geometry_are_checked_before_publication() {
    let mut state = State::open(VectorMemory::default(), true);
    let (mut manifest, bytes) = fixture("bad-hash", 10);
    manifest.hash[0] ^= 1;
    state.prepare(&manifest).unwrap();
    upload(&mut state, &manifest, &bytes);
    assert_eq!(state.publish(&manifest.release), Err(Error::Hash));
    assert!(!state.status(&manifest.release).unwrap().published);
    let invalid = Manifest {
        bytes: u64::MAX,
        ..manifest.clone()
    };
    assert_eq!(state.prepare(&invalid), Err(Error::Invalid));
    let invalid = Release {
        name: "app\0other".into(),
        version: "1".into(),
    };
    assert_eq!(state.status(&invalid), Err(Error::Invalid));
}

#[test]
fn history_pages_are_bounded_and_keep_all_versions() {
    let mut state = State::open(VectorMemory::default(), true);
    for i in 0..20 {
        let (manifest, _) = fixture(&format!("{i:02}"), 10);
        state.prepare(&manifest).unwrap();
    }
    let mut after = None;
    let mut seen = Vec::new();
    loop {
        let page = state.list(after.as_ref()).unwrap();
        assert!(page.len() <= PAGE_SIZE);
        if page.is_empty() {
            break;
        }
        after = Some(page.last().unwrap().manifest.release.clone());
        seen.extend(
            page.into_iter()
                .map(|status| status.manifest.release.version),
        );
    }
    assert_eq!(seen, (0..20).map(|i| format!("{i:02}")).collect::<Vec<_>>());
    assert_eq!(state.info().artifacts, 1);
    assert_eq!(state.info().reserved_bytes, 10);
}

#[test]
fn pins_keep_exact_owner_operation_and_release_across_reopen() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (manifest, bytes) = fixture("1", 10);
    let pin = Pin {
        operation: [1; 32],
        manifest: manifest.clone(),
    };
    state.prepare(&manifest).unwrap();
    assert_eq!(state.pin(&[1], &pin), Err(Error::Incomplete));
    upload(&mut state, &manifest, &bytes);
    state.publish(&manifest.release).unwrap();
    assert_eq!(state.observe_pin(&[1], &pin), Err(Error::NotFound));
    let receipt = state.pin(&[1], &pin).unwrap();
    assert_eq!(state.pin(&[1], &pin).unwrap(), receipt);
    assert_eq!(state.observe_pin(&[2], &pin), Err(Error::NotFound));
    let other = Manifest {
        release: Release {
            version: "2".into(),
            ..manifest.release.clone()
        },
        ..manifest
    };
    state.prepare(&other).unwrap();
    assert_eq!(
        state.pin(
            &[1],
            &Pin {
                manifest: other,
                ..pin.clone()
            }
        ),
        Err(Error::Conflict)
    );
    drop(state);
    let state = State::open(backing, false);
    assert_eq!(state.observe_pin(&[1], &pin).unwrap(), receipt);
}

#[test]
fn pin_capacity_never_evicts_and_old_artifact_schema_gains_no_authority() {
    use super::records::CheckedRecord;
    use std::borrow::Cow;
    #[derive(serde::Serialize)]
    struct Legacy {
        bytes: u64,
        chunks: Vec<[u8; 32]>,
        published: bool,
    }
    let old = Legacy {
        bytes: 8,
        chunks: vec![[3; 32]],
        published: true,
    };
    let mut encoded = b"CWA1".to_vec();
    encoded.extend(
        ic_wire::encode(
            &old,
            ic_wire::Encoding::Payload,
            ic_wasm_store_protocol::LIMITS,
        )
        .unwrap(),
    );
    let decoded = Artifact::decode_record(Cow::Owned(encoded)).unwrap();
    assert!(decoded.published);
    assert!(decoded.pins.is_empty());
    let round_trip = Artifact::decode_record(decoded.encode_record().unwrap()).unwrap();
    assert!(round_trip.pins.is_empty());

    let mut state = State::open(VectorMemory::default(), true);
    let (mut manifest, bytes) = fixture(&"v".repeat(64), 10);
    manifest.release.name = "n".repeat(64);
    state.prepare(&manifest).unwrap();
    upload(&mut state, &manifest, &bytes);
    state.publish(&manifest.release).unwrap();
    for i in 0..MAX_PINS {
        let pin = Pin {
            operation: [i as u8 + 1; 32],
            manifest: manifest.clone(),
        };
        state.pin(&[255; 29], &pin).unwrap();
    }
    let first = Pin {
        operation: [1; 32],
        manifest: manifest.clone(),
    };
    assert!(state.pin(&[255; 29], &first).is_ok());
    assert_eq!(
        state.pin(
            &[255; 29],
            &Pin {
                operation: [255; 32],
                manifest
            }
        ),
        Err(Error::Capacity)
    );
    assert!(state.observe_pin(&[255; 29], &first).is_ok());
    let mut maximum = state.artifacts.get(&first.manifest.hash).unwrap();
    maximum.bytes = (CHUNK_BYTES * ic_wasm_store_protocol::MAX_CHUNKS) as u64;
    maximum.chunks = vec![[255; 32]; ic_wasm_store_protocol::MAX_CHUNKS];
    let encoded = ic_storage::adapter::Storable::to_bytes_checked(&maximum);
    let restored = Artifact::decode_record(encoded).unwrap();
    assert_eq!(restored.pins.len(), MAX_PINS);
    assert_eq!(restored.chunks, maximum.chunks);
}

#[test]
fn deletion_and_cancellation_preserve_identity_and_reclaim_only_unshared_payloads() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (manifest, bytes) = fixture("delete", CHUNK_BYTES + 8);
    state.prepare(&manifest).unwrap();
    upload(&mut state, &manifest, &bytes);
    state.publish(&manifest.release).unwrap();
    let alias = Manifest {
        release: Release {
            version: "alias".into(),
            ..manifest.release.clone()
        },
        ..manifest.clone()
    };
    state.prepare(&alias).unwrap();
    assert_eq!(state.remove_release(&manifest, false), Err(Error::Conflict));
    state.remove_release(&manifest, true).unwrap();
    assert_eq!(state.status(&manifest.release), Err(Error::NotFound));
    assert!(state.status(&alias.release).unwrap().published);
    assert_eq!(state.info().reserved_bytes, manifest.bytes);
    state.remove_release(&alias, true).unwrap();
    assert_eq!(state.info().reserved_bytes, 0);
    assert_eq!(state.info().chunks, 0);
    assert_eq!(state.tombstones(None).unwrap().len(), 2);
    assert_eq!(state.remove_release(&manifest, true).unwrap(), manifest);
    drop(state);
    let mut state = State::open(backing, false);
    let mut conflicting = manifest.clone();
    conflicting.hash[0] ^= 1;
    assert_eq!(state.prepare(&conflicting), Err(Error::Conflict));
    let restored = state.prepare(&manifest).unwrap();
    assert!(!restored.published);
    assert_eq!(restored.missing, vec![0, 1]);
    upload(&mut state, &manifest, &bytes);
    state.remove_release(&manifest, false).unwrap();
    assert_eq!(state.info().chunks, 0);
    assert_eq!(state.info().reserved_bytes, 0);
    state.prepare(&manifest).unwrap();
    upload(&mut state, &manifest, &bytes);
    state.publish(&manifest.release).unwrap();
    let pin = Pin {
        operation: [1; 32],
        manifest: manifest.clone(),
    };
    state.pin(&[1], &pin).unwrap();
    assert_eq!(state.remove_release(&manifest, true), Err(Error::Conflict));
    state.observe_pin(&[1], &pin).unwrap();
}

#[test]
fn cancellation_retains_cross_artifact_chunks_and_respects_admission_and_label_quota() {
    let mut state = State::open(VectorMemory::default(), true);
    let (first, bytes) = fixture("first", CHUNK_BYTES + 8);
    let (second, other) = fixture("second", CHUNK_BYTES + 9);
    state.prepare(&first).unwrap();
    state.prepare(&second).unwrap();
    upload(&mut state, &first, &bytes);
    upload(&mut state, &second, &other);
    assert_eq!(state.info().chunks, 3);
    state.remove_release(&first, false).unwrap();
    assert_eq!(state.info().chunks, 2);
    state.publish(&second.release).unwrap();
    state
        .configure(&Quota {
            releases: 2,
            reserved_bytes: 32 * 1024 * 1024,
        })
        .unwrap();
    let (third, _) = fixture("third", 8);
    assert_eq!(state.prepare(&third), Err(Error::Capacity));
    assert!(state.prepare(&first).is_ok());
    let attempt = ic_wasm_store_protocol::MaintenanceAttempt {
        sequence: 1,
        operation: [2; 32],
    };
    state
        .admission(
            &[1],
            &ic_wasm_store_protocol::AdmissionCommand::Close(attempt),
        )
        .unwrap();
    assert_eq!(state.remove_release(&first, false), Err(Error::Closed));
    assert_eq!(state.remove_release(&second, true), Err(Error::Closed));
}

#[test]
fn four_allocation_store_reopens_without_reinitializing_existing_records() {
    let backing = VectorMemory::default();
    let mut runtime = MemoryRuntime::new_with_config(
        backing.clone(),
        StorageConfig {
            allocations: 4,
            extents: 4096,
            pages: 16_384,
            ..StorageConfig::default()
        },
    )
    .unwrap();
    let names = [
        "store.releases.v1",
        "store.artifacts.v1",
        "store.chunks.v1",
        "store.settings.v1",
    ];
    let declarations = SealedDeclarationSnapshot::new(
        names
            .iter()
            .map(|name| MemoryRequest::new("store", name).unwrap())
            .collect(),
    )
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("store", "store.").unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let mut memories = names
        .map(|name| {
            runtime
                .open_memory(&StableKey::parse(name).unwrap())
                .unwrap()
        })
        .into_iter();
    let mut releases = StableBTreeMap::<Key, [u8; 32], _>::create(memories.next().unwrap());
    let mut artifacts = StableBTreeMap::<[u8; 32], Artifact, _>::create(memories.next().unwrap());
    let mut chunks = StableBTreeMap::<[u8; 32], Payload, _>::create(memories.next().unwrap());
    let (manifest, bytes) = fixture("pre-removal", 8);
    releases.insert(Key::new(&manifest.release), manifest.hash);
    artifacts.insert(
        manifest.hash,
        Artifact {
            bytes: 8,
            chunks: manifest.chunks.clone(),
            published: true,
            pins: vec![],
        },
    );
    chunks.insert(manifest.chunks[0], Payload(bytes));
    let settings = StableCell::create(
        memories.next().unwrap(),
        Settings {
            quota: Quota::default(),
            reserved_bytes: 8,
            admission: None,
        },
    );
    drop((releases, artifacts, chunks, settings, runtime));
    let mut state = State::open(backing.clone(), false);
    assert!(state.status(&manifest.release).unwrap().published);
    assert!(state.tombstones(None).unwrap().is_empty());
    state.remove_release(&manifest, true).unwrap();
    drop(state);
    let state = State::open(backing, false);
    assert_eq!(state.tombstones(None).unwrap(), vec![manifest]);
}

#[test]
fn preservation_enumerates_tombstones_and_active_aliases_once() {
    use ic_wasm_store_protocol::{AdmissionCommand, MaintenanceAttempt, preservation::Request};
    let mut state = State::open(VectorMemory::default(), true);
    for version in ["a", "b", "c"] {
        let (manifest, _) = fixture(version, 8);
        state.prepare(&manifest).unwrap();
        if version != "b" {
            state.remove_release(&manifest, false).unwrap();
        }
    }
    let attempt = MaintenanceAttempt {
        sequence: 1,
        operation: [1; 32],
    };
    state
        .admission(&[1], &AdmissionCommand::Close(attempt.clone()))
        .unwrap();
    let mut after = None;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..3 {
        let page = state
            .preservation(
                &[1],
                &Request {
                    attempt: attempt.clone(),
                    after,
                },
            )
            .unwrap();
        assert_eq!(page.catalog.releases, 3);
        let release = page.entry.unwrap().status.manifest.release;
        assert!(seen.insert(release.version.clone()));
        after = Some(release);
    }
    assert!(
        state
            .preservation(&[1], &Request { attempt, after })
            .unwrap()
            .entry
            .is_none()
    );
}

#[test]
fn restoration_capacity_refusal_preserves_tombstone_across_reopen() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (removed, bytes) = fixture("removed", 10);
    let (occupant, other) = fixture("occupant", 11);
    state
        .configure(&Quota {
            releases: 2,
            reserved_bytes: 11,
        })
        .unwrap();
    state.prepare(&removed).unwrap();
    upload(&mut state, &removed, &bytes);
    state.publish(&removed.release).unwrap();
    state.remove_release(&removed, true).unwrap();
    state.prepare(&occupant).unwrap();
    upload(&mut state, &occupant, &other);
    state.publish(&occupant.release).unwrap();
    let before = state.info();
    for _ in 0..2 {
        assert_eq!(state.prepare(&removed), Err(Error::Capacity));
        assert_eq!(state.info(), before);
        assert_eq!(state.tombstones(None).unwrap(), vec![removed.clone()]);
        assert_eq!(state.status(&removed.release), Err(Error::NotFound));
        assert_eq!(
            state.put(&Put {
                release: removed.release.clone(),
                index: 0,
                bytes: bytes.clone()
            }),
            Err(Error::NotFound)
        );
        assert_eq!(state.publish(&removed.release), Err(Error::NotFound));
        assert!(state.status(&occupant.release).unwrap().published);
        drop(state);
        state = State::open(backing.clone(), false);
    }
    state.remove_release(&occupant, true).unwrap();
    let restored = state.prepare(&removed).unwrap();
    assert_eq!(restored.missing, vec![0]);
    assert!(!restored.published);
    assert_eq!(state.info().reserved_bytes, removed.bytes);
    upload(&mut state, &removed, &bytes);
    state.publish(&removed.release).unwrap();
    drop(state);
    let state = State::open(backing, false);
    assert!(state.status(&removed.release).unwrap().published);
    assert_eq!(state.tombstones(None).unwrap(), vec![occupant]);
}

#[test]
fn tombstone_pages_preserve_mixed_names_and_versions_across_reopen() {
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let mut expected = std::collections::BTreeSet::new();
    // Prefix-related names and interleaved active/removed versions exercise
    // pagination across both catalog key separators without assuming their order.
    for name in ["app", "app-a", "app.z"] {
        for version in 0..6 {
            let (mut manifest, _) = fixture(&format!("{version}"), 8);
            manifest.release.name = name.into();
            state.prepare(&manifest).unwrap();
            if version % 3 != 1 {
                state.remove_release(&manifest, false).unwrap();
                expected.insert((name.to_string(), manifest.release.version.clone()));
            }
        }
    }
    drop(state);
    let state = State::open(backing, false);
    let mut after = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut finished = false;
    for _ in 0..=expected.len() {
        let page = state.tombstones(after.as_ref()).unwrap();
        assert!(page.len() <= PAGE_SIZE);
        if page.is_empty() {
            finished = true;
            break;
        }
        after = Some(page.last().unwrap().release.clone());
        for manifest in page {
            assert_eq!(state.status(&manifest.release), Err(Error::NotFound));
            assert!(seen.insert((manifest.release.name, manifest.release.version)));
        }
    }
    assert!(finished, "pagination must terminate");
    assert_eq!(seen, expected);
    assert_eq!(state.info().releases, 18);
    assert_eq!(state.info().reserved_bytes, 8);
    assert_eq!(
        state.tombstones(Some(&Release {
            name: String::new(),
            version: "0".into()
        })),
        Err(Error::Invalid)
    );
}

#[test]
fn preservation_v2_distinguishes_restoration_and_retains_order_across_reopen() {
    use ic_wasm_store_protocol::{
        AdmissionCommand, MaintenanceAttempt,
        preservation::{Entry, Request},
    };
    let backing = VectorMemory::default();
    let mut state = State::open(backing.clone(), true);
    let (removed, _) = fixture("a", 33);
    let alias = Manifest {
        release: Release {
            version: "z".into(),
            ..removed.release.clone()
        },
        ..removed.clone()
    };
    state.prepare(&removed).unwrap();
    state.prepare(&alias).unwrap();
    state.remove_release(&removed, false).unwrap();
    let attempt = MaintenanceAttempt {
        sequence: 1,
        operation: [11; 32],
    };
    let request = Request {
        attempt: attempt.clone(),
        after: None,
    };
    assert_eq!(state.preservation_v2(&[1], &request), Err(Error::Conflict));
    state
        .admission(&[1], &AdmissionCommand::Close(attempt.clone()))
        .unwrap();
    assert_eq!(state.preservation_v2(&[2], &request), Err(Error::Conflict));
    let first = state.preservation_v2(&[1], &request).unwrap();
    let next = Request {
        after: Some(alias.release.clone()),
        ..request.clone()
    };
    let second = state.preservation_v2(&[1], &next).unwrap();
    assert!(matches!(first.entry, Some(Entry::Active(_))));
    assert_eq!(second.entry, Some(Entry::Removed(removed.clone())));
    assert!(first.entry.as_ref().unwrap().order_key() < second.entry.as_ref().unwrap().order_key());
    assert_eq!(first.catalog, second.catalog);
    let end = Request {
        after: Some(removed.release.clone()),
        ..request.clone()
    };
    assert!(state.preservation_v2(&[1], &end).unwrap().entry.is_none());
    drop(state);
    let mut state = State::open(backing, false);
    assert_eq!(state.preservation_v2(&[1], &request).unwrap(), first);
    assert_eq!(state.preservation_v2(&[1], &next).unwrap(), second);
    state
        .admission(&[1], &AdmissionCommand::Activate(attempt))
        .unwrap();
    // All previous removals have returned before explicit restoration.
    state.prepare(&removed).unwrap();
    let attempt = MaintenanceAttempt {
        sequence: 2,
        operation: [12; 32],
    };
    state
        .admission(&[1], &AdmissionCommand::Close(attempt.clone()))
        .unwrap();
    assert_eq!(state.preservation_v2(&[1], &request), Err(Error::Conflict));
    let restored = state
        .preservation_v2(
            &[1],
            &Request {
                attempt,
                after: None,
            },
        )
        .unwrap();
    assert_eq!(restored.catalog, second.catalog);
    assert_eq!(restored.entry.as_ref().unwrap().manifest(), &removed);
    assert!(matches!(restored.entry, Some(Entry::Active(_))));
    assert_ne!(restored.entry, second.entry);
}
