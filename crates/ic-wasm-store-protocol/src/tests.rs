use super::*;

#[test]
fn maximum_metadata_page_and_chunk_fit_independent_wire_budgets() {
    let manifest = Manifest {
        release: Release {
            name: "a".repeat(64),
            version: "v".repeat(64),
        },
        hash: [255; 32],
        bytes: (CHUNK_BYTES * MAX_CHUNKS) as u64,
        chunks: vec![[255; 32]; MAX_CHUNKS],
    };
    assert!(manifest.valid());
    let pin = Pin {
        operation: [255; 32],
        manifest: manifest.clone(),
    };
    let receipt = Ok::<_, Error>(PinReceipt {
        owner: vec![255; 29],
        pin: pin.clone(),
    });
    let encoded = ic_wire::encode(
        &receipt,
        ic_wire::Encoding::Frame {
            method: PIN,
            direction: ic_wire::Direction::Response,
        },
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        ic_wire::decode::<Result<PinReceipt, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PIN,
                direction: ic_wire::Direction::Response
            },
            LIMITS
        )
        .unwrap(),
        receipt
    );
    let request = PrepareCache {
        pin,
        expires_at_ns: u64::MAX,
    };
    let encoded = ic_wire::encode(
        &request,
        ic_wire::Encoding::Frame {
            method: PREPARE_CACHE,
            direction: ic_wire::Direction::Request,
        },
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        ic_wire::decode::<PrepareCache>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PREPARE_CACHE,
                direction: ic_wire::Direction::Request
            },
            LIMITS
        )
        .unwrap(),
        request
    );
    let status = Status {
        manifest: manifest.clone(),
        published: false,
        missing: (0..MAX_CHUNKS as u32).collect(),
    };
    let custody = Ok::<_, Error>(Custody {
        status: Status {
            published: true,
            missing: vec![],
            ..status.clone()
        },
        pins: (0..MAX_PINS)
            .map(|i| Retention {
                owner: vec![255; 29],
                operation: [i as u8 + 1; 32],
                release: manifest.release.clone(),
            })
            .collect(),
    });
    let encoded = ic_wire::encode(
        &custody,
        ic_wire::Encoding::Frame {
            method: CUSTODY,
            direction: ic_wire::Direction::Response,
        },
        LIMITS,
    )
    .unwrap();
    let decoded = ic_wire::decode::<Result<Custody, Error>>(
        &encoded,
        ic_wire::Encoding::Frame {
            method: CUSTODY,
            direction: ic_wire::Direction::Response,
        },
        LIMITS,
    )
    .unwrap();
    assert_eq!(decoded, custody);
    let catalog = preservation::Catalog {
        quota: Quota {
            reserved_bytes: u64::MAX,
            releases: u64::MAX,
        },
        reserved_bytes: u64::MAX,
        releases: u64::MAX,
        artifacts: u64::MAX,
        chunks: u64::MAX,
    };
    for entry in [
        preservation::Entry::Active(decoded.unwrap()),
        preservation::Entry::Removed(manifest.clone()),
    ] {
        let page = Ok::<_, Error>(preservation::Page {
            catalog: catalog.clone(),
            entry: Some(entry),
        });
        let encoded = ic_wire::encode(
            &page,
            ic_wire::Encoding::Frame {
                method: PRESERVATION,
                direction: ic_wire::Direction::Response,
            },
            LIMITS,
        )
        .unwrap();
        assert_eq!(
            ic_wire::decode::<Result<preservation::Page, Error>>(
                &encoded,
                ic_wire::Encoding::Frame {
                    method: PRESERVATION,
                    direction: ic_wire::Direction::Response
                },
                LIMITS
            )
            .unwrap(),
            page
        );
        assert_eq!(
            ic_wire::decode::<Result<preservation::Page, Error>>(
                &encoded,
                ic_wire::Encoding::Frame {
                    method: 113,
                    direction: ic_wire::Direction::Response
                },
                LIMITS
            )
            .unwrap_err(),
            ic_wire::Error::Identity
        );
    }
    let page = Ok::<_, Error>(vec![status; PAGE_SIZE]);
    let encoded = ic_wire::encode(
        &page,
        ic_wire::Encoding::Frame {
            method: LIST,
            direction: ic_wire::Direction::Response,
        },
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        ic_wire::decode::<Result<Vec<Status>, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: LIST,
                direction: ic_wire::Direction::Response
            },
            LIMITS
        )
        .unwrap(),
        page
    );
    let input = Put {
        release: manifest.release,
        index: 0,
        bytes: vec![255; CHUNK_BYTES],
    };
    let encoded = ic_wire::encode(
        &input,
        ic_wire::Encoding::Frame {
            method: PUT,
            direction: ic_wire::Direction::Request,
        },
        CHUNK_LIMITS,
    )
    .unwrap();
    assert_eq!(
        ic_wire::decode::<Put>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PUT,
                direction: ic_wire::Direction::Request
            },
            CHUNK_LIMITS
        )
        .unwrap()
        .bytes
        .len(),
        CHUNK_BYTES
    );
    assert_eq!(
        ic_wire::decode::<Put>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PUT,
                direction: ic_wire::Direction::Request
            },
            LIMITS
        )
        .unwrap_err(),
        ic_wire::Error::Limit
    );
    let response = Ok::<_, Error>(Chunk(input.bytes));
    let encoded = ic_wire::encode(
        &response,
        ic_wire::Encoding::Frame {
            method: READ,
            direction: ic_wire::Direction::Response,
        },
        CHUNK_LIMITS,
    )
    .unwrap();
    assert_eq!(
        ic_wire::decode::<Result<Chunk, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: READ,
                direction: ic_wire::Direction::Response
            },
            CHUNK_LIMITS
        )
        .unwrap(),
        response
    );
}
