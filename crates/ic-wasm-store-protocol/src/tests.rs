use super::*;

use ic_wire::{Encoding, Error as WireError, Limits, Message, evolution::inline::Inline};

fn encode<T: for<'de> Inline<'de>>(
    value: T,
    encoding: Encoding,
    limits: Limits,
) -> Result<Vec<u8>, WireError> {
    ic_wire::encode(&Message(value), encoding, limits)
}

fn decode<'de, T: Inline<'de>>(
    bytes: &'de [u8],
    encoding: Encoding,
    limits: Limits,
) -> Result<T, WireError> {
    ic_wire::decode::<Message<T>>(bytes, encoding, limits).map(|message| message.0)
}
const fn wire_limits(budget: FrameBudget) -> ic_wire::Limits {
    ic_wire::Limits {
        bytes: budget.bytes,
        allocation: budget.allocation,
        nodes: budget.nodes,
        depth: budget.depth,
    }
}

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
    let encoded = encode(
        receipt.clone(),
        ic_wire::Encoding::Frame {
            method: PIN,
            direction: ic_wire::Direction::Response,
        },
        wire_limits(LIMITS),
    )
    .unwrap();
    assert_eq!(
        decode::<Result<PinReceipt, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PIN,
                direction: ic_wire::Direction::Response
            },
            wire_limits(LIMITS)
        )
        .unwrap(),
        receipt
    );
    let request = PrepareCache {
        pin,
        expires_at_ns: u64::MAX,
    };
    let encoded = encode(
        request.clone(),
        ic_wire::Encoding::Frame {
            method: PREPARE_CACHE,
            direction: ic_wire::Direction::Request,
        },
        wire_limits(LIMITS),
    )
    .unwrap();
    assert_eq!(
        decode::<PrepareCache>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PREPARE_CACHE,
                direction: ic_wire::Direction::Request
            },
            wire_limits(LIMITS)
        )
        .unwrap(),
        request
    );
    let status = Status {
        manifest: manifest.clone(),
        published: false,
        missing: (0..MAX_CHUNKS as u32).collect(),
    };
    let custody = Ok::<_, Error>(Observation::Custody(Custody {
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
    }));
    let encoded = encode(
        custody.clone(),
        ic_wire::Encoding::Frame {
            method: OBSERVE,
            direction: ic_wire::Direction::Response,
        },
        wire_limits(LIMITS),
    )
    .unwrap();
    let decoded = decode::<Result<Observation, Error>>(
        &encoded,
        ic_wire::Encoding::Frame {
            method: OBSERVE,
            direction: ic_wire::Direction::Response,
        },
        wire_limits(LIMITS),
    )
    .unwrap();
    assert_eq!(decoded, custody);
    let Observation::Custody(verified) = decoded.unwrap() else {
        panic!("expected release custody")
    };
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
        preservation::Entry::Active(verified),
        preservation::Entry::Removed(manifest.clone()),
    ] {
        let page = Ok::<_, Error>(preservation::Page {
            catalog: catalog.clone(),
            entry: Some(entry),
        });
        let encoded = encode(
            page.clone(),
            ic_wire::Encoding::Frame {
                method: PRESERVATION,
                direction: ic_wire::Direction::Response,
            },
            wire_limits(LIMITS),
        )
        .unwrap();
        assert_eq!(
            decode::<Result<preservation::Page, Error>>(
                &encoded,
                ic_wire::Encoding::Frame {
                    method: PRESERVATION,
                    direction: ic_wire::Direction::Response
                },
                wire_limits(LIMITS)
            )
            .unwrap(),
            page
        );
        assert_eq!(
            decode::<Result<preservation::Page, Error>>(
                &encoded,
                ic_wire::Encoding::Frame {
                    method: 113,
                    direction: ic_wire::Direction::Response
                },
                wire_limits(LIMITS)
            )
            .unwrap_err(),
            ic_wire::Error::Identity
        );
    }
    let page = Ok::<_, Error>(Inspection::List(vec![status; PAGE_SIZE]));
    let encoded = encode(
        page.clone(),
        ic_wire::Encoding::Frame {
            method: INSPECT,
            direction: ic_wire::Direction::Response,
        },
        wire_limits(LIMITS),
    )
    .unwrap();
    assert_eq!(
        decode::<Result<Inspection, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: INSPECT,
                direction: ic_wire::Direction::Response
            },
            wire_limits(LIMITS)
        )
        .unwrap(),
        page
    );
    let input = Put {
        release: manifest.release,
        index: 0,
        bytes: vec![255; CHUNK_BYTES],
    };
    let encoded = encode(
        input.clone(),
        ic_wire::Encoding::Frame {
            method: PUT,
            direction: ic_wire::Direction::Request,
        },
        wire_limits(CHUNK_LIMITS),
    )
    .unwrap();
    assert_eq!(
        decode::<Put>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PUT,
                direction: ic_wire::Direction::Request
            },
            wire_limits(CHUNK_LIMITS)
        )
        .unwrap()
        .bytes
        .len(),
        CHUNK_BYTES
    );
    assert_eq!(
        decode::<Put>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: PUT,
                direction: ic_wire::Direction::Request
            },
            wire_limits(LIMITS)
        )
        .unwrap_err(),
        ic_wire::Error::Limit(ic_wire::LimitKind::Bytes)
    );
    let response = Ok::<_, Error>(Chunk(input.bytes));
    let encoded = encode(
        response.clone(),
        ic_wire::Encoding::Frame {
            method: READ,
            direction: ic_wire::Direction::Response,
        },
        wire_limits(CHUNK_LIMITS),
    )
    .unwrap();
    assert_eq!(
        decode::<Result<Chunk, Error>>(
            &encoded,
            ic_wire::Encoding::Frame {
                method: READ,
                direction: ic_wire::Direction::Response
            },
            wire_limits(CHUNK_LIMITS)
        )
        .unwrap(),
        response
    );
}
