//! Versioned metadata and independently bounded chunk payloads.

use ic_storage::adapter::{Bound, Storable};
use ic_wasm_store_protocol::{
    CHUNK_BYTES, LIMITS, MAX_CHUNKS, MAX_PINS, MAX_READ_GRANTS, Manifest, Quota, Release,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;

pub(super) trait CheckedRecord: Sized {
    const BOUND: Bound;
    fn encode_record(&self) -> Result<Cow<'_, [u8]>, &'static str>;
    fn decode_record(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str>;
}

// The substrate traps on malformed persisted values. Keep Store's codec validation
// explicit at that boundary so invalid records cannot be reopened as defaults.
macro_rules! stable_record {
    ($record:ty) => {
        impl Storable for $record {
            const BOUND: Bound = <Self as CheckedRecord>::BOUND;

            fn to_bytes(&self) -> Cow<'_, [u8]> {
                self.encode_record().expect("invalid Store record")
            }

            fn into_bytes(self) -> Vec<u8> {
                self.to_bytes().into_owned()
            }

            fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
                Self::decode_record(bytes).expect("invalid Store record")
            }
        }
    };
}

///
/// Key
/// Encodes release labels, with separate separators for active entries and tombstones.
///

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Key(pub String);

impl Key {
    pub fn new(release: &Release) -> Self {
        Self(format!("{}\0{}", release.name, release.version))
    }

    pub fn tombstone(release: &Release) -> Self {
        Self(format!("{}\x01{}", release.name, release.version))
    }

    pub fn is_tombstone(&self) -> bool {
        self.0.contains('\x01')
    }

    pub fn release(&self) -> Release {
        let (name, version) = self
            .0
            .split_once(['\0', '\x01'])
            .expect("validated release key");

        Release {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl CheckedRecord for Key {
    const BOUND: Bound = Bound::Bounded {
        max_size: 129,
        is_fixed_size: false,
    };

    fn encode_record(&self) -> Result<Cow<'_, [u8]>, &'static str> {
        Ok(Cow::Borrowed(self.0.as_bytes()))
    }

    fn decode_record(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
        let key = String::from_utf8(bytes.into_owned()).map_err(|_| "release key")?;
        let (name, version) = key.split_once(['\0', '\x01']).ok_or("release key")?;
        if !(Release {
            name: name.into(),
            version: version.into(),
        })
        .valid()
        {
            return Err("release labels");
        }

        Ok(Self(key))
    }
}

stable_record!(Key);

///
/// Artifact
/// Retains artifact geometry, publication state and pins in the artifact allocation.
///

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Artifact {
    pub hash: [u8; 32],
    pub bytes: u64,
    pub chunks: Vec<[u8; 32]>,
    pub published: bool,
    pub pins: Vec<RetainedPin>,
}

impl Artifact {
    // Fixed-width fields followed by an ordered sequence of fixed-width chunk hashes.
    // Exclude the label so exact aliases still share one reservation and pin set.
    pub fn identity(manifest: &Manifest) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"Store artifact geometry v1");
        digest.update(manifest.hash);
        digest.update(manifest.bytes.to_be_bytes());
        for chunk in &manifest.chunks {
            digest.update(chunk);
        }

        digest.finalize().into()
    }

    fn valid(&self) -> bool {
        self.bytes >= 8
            && self.bytes <= (CHUNK_BYTES * MAX_CHUNKS) as u64
            && self.chunks.len() == (self.bytes as usize).div_ceil(CHUNK_BYTES)
            && self.pins.len() <= MAX_PINS
            && self.pins.iter().all(RetainedPin::valid)
            && (self.pins.is_empty() || self.published)
            && self.pins.iter().enumerate().all(|(i, pin)| {
                !self.pins[..i]
                    .iter()
                    .any(|prior| prior.owner == pin.owner && prior.operation == pin.operation)
            })
    }

    pub fn chunk_bytes(&self, index: usize) -> usize {
        (self.bytes as usize - index * CHUNK_BYTES).min(CHUNK_BYTES)
    }
}

impl CheckedRecord for Artifact {
    const BOUND: Bound = METADATA_BOUND;

    fn encode_record(&self) -> Result<Cow<'_, [u8]>, &'static str> {
        if !self.valid() {
            return Err("artifact invariant");
        }

        encode(self, b"CWA4")
    }

    fn decode_record(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
        let payload = bytes.strip_prefix(b"CWA4").ok_or("artifact schema")?;
        let value: Self = ic_wire::decode(payload, ic_wire::Encoding::Payload, RECORD_LIMITS)
            .map_err(|_| "artifact record")?;
        if !value.valid() {
            return Err("artifact invariant");
        }

        Ok(value)
    }
}

stable_record!(Artifact);

///
/// Settings
/// Retains quota, reserved bytes and maintenance admission across upgrades.
///

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Settings {
    pub quota: Quota,
    pub reserved_bytes: u64,
    pub admission: Option<ic_wasm_store_protocol::Admission>,
    pub read_grants: Vec<RetainedReadGrant>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct RetainedReadGrant {
    pub installer: Vec<u8>,
    pub issuer: Vec<u8>,
    pub fingerprint: [u8; 32],
    pub expires_at_ns: u64,
}

impl Settings {
    fn valid(&self) -> bool {
        self.quota.valid()
            && self.reserved_bytes <= self.quota.reserved_bytes
            && self.admission.as_ref().is_none_or(|gate| {
                gate.attempt.sequence > 0
                    && gate.attempt.operation != [0; 32]
                    && !gate.owner.is_empty()
                    && gate.owner.len() <= 29
                    && gate.owner != [4]
            })
            && self.read_grants.len() <= MAX_READ_GRANTS
            && self
                .admission
                .as_ref()
                .is_none_or(|gate| !gate.closed || self.read_grants.is_empty())
            && self.read_grants.iter().enumerate().all(|(index, grant)| {
                [&grant.installer, &grant.issuer].iter().all(|principal| {
                    !principal.is_empty() && principal.len() <= 29 && principal.as_slice() != [4]
                }) && grant.expires_at_ns > 0
                    && !self.read_grants[..index]
                        .iter()
                        .any(|prior| prior.fingerprint == grant.fingerprint)
            })
    }
}

impl CheckedRecord for Settings {
    const BOUND: Bound = METADATA_BOUND;

    fn encode_record(&self) -> Result<Cow<'_, [u8]>, &'static str> {
        if !self.valid() {
            return Err("settings invariant");
        }

        encode(self, b"CWS6")
    }

    fn decode_record(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
        let payload = bytes.strip_prefix(b"CWS6").ok_or("settings schema")?;
        let value: Self = ic_wire::decode(payload, ic_wire::Encoding::Payload, RECORD_LIMITS)
            .map_err(|_| "settings record")?;
        if !value.valid() {
            return Err("settings invariant");
        }

        Ok(value)
    }
}

stable_record!(Settings);

///
/// RetainedPin
/// Binds an artifact to the exact owner, operation and release without a separate allocation.
///

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct RetainedPin {
    pub owner: Vec<u8>,
    pub operation: [u8; 32],
    pub release: Release,
}

impl RetainedPin {
    fn valid(&self) -> bool {
        !self.owner.is_empty()
            && self.owner.len() <= 29
            && self.owner != [4]
            && self.operation != [0; 32]
            && self.release.valid()
    }
}

const RECORD_LIMITS: ic_wire::Limits = ic_wire::Limits {
    allocation: 1_048_576,
    nodes: 65_536,
    ..LIMITS
};
const METADATA_BOUND: Bound = Bound::Bounded {
    max_size: 16_384,
    is_fixed_size: false,
};

fn encode<T: Serialize>(value: &T, tag: &[u8]) -> Result<Cow<'static, [u8]>, &'static str> {
    let mut bytes = tag.to_vec();
    bytes.extend(
        ic_wire::encode(value, ic_wire::Encoding::Payload, RECORD_LIMITS)
            .map_err(|_| "store record")?,
    );

    Ok(Cow::Owned(bytes))
}

///
/// Payload
/// Stores one verified Wasm chunk as raw stable bytes.
///

pub(super) struct Payload(pub Vec<u8>);

impl CheckedRecord for Payload {
    const BOUND: Bound = Bound::Bounded {
        max_size: CHUNK_BYTES as u32,
        is_fixed_size: false,
    };

    fn encode_record(&self) -> Result<Cow<'_, [u8]>, &'static str> {
        Ok(Cow::Borrowed(&self.0))
    }

    fn decode_record(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
        if bytes.is_empty() || bytes.len() > CHUNK_BYTES {
            return Err("chunk size");
        }

        Ok(Self(bytes.into_owned()))
    }
}

stable_record!(Payload);
