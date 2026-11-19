//! Bounded metadata enumeration under one closed admission attempt. No payload integrity claim.
use crate::{Custody, MaintenanceAttempt, Quota, Release};
use serde::{Deserialize, Serialize};

/// Caller must own the exact currently closed attempt on every page.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Request {
    /// Exact closure, never inferred from current state.
    pub attempt: MaintenanceAttempt,
    /// Previous returned release; None starts enumeration.
    pub after: Option<Release>,
}

/// Stable metadata counters, deliberately excluding executable identity and cache state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Catalog {
    /// Configured limits, including incomplete releases.
    pub quota: Quota,
    /// Reserved payload capacity, not physical backing usage.
    pub reserved_bytes: u64,
    /// Retained release count, used to detect incomplete enumeration.
    pub releases: u64,
    /// Distinct artifacts shared by those releases.
    pub artifacts: u64,
    /// Distinct retained payload chunks; this read does not hash their bytes.
    pub chunks: u64,
}

/// One complete release record and its shared pins; None establishes end of enumeration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Page {
    /// Must remain identical for every page under the selected closure.
    pub catalog: Catalog,
    /// At most one release, including incomplete uploads and tombstones.
    /// Tombstones appear unpublished with every chunk missing and no pins;
    /// the TOMBSTONES endpoint distinguishes them from active uploads.
    pub entry: Option<Custody>,
}

/// Exact retained label state; removal is distinct from an empty upload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Entry {
    /// Active upload or published release, including all shared artifact pins.
    Active(Custody),
    /// Permanently retained removed identity, still charged against label quota.
    Removed(crate::Manifest),
}
impl Entry {
    /// Immutable manifest, whether active or removed.
    #[must_use]
    pub fn manifest(&self) -> &crate::Manifest {
        match self {
            Self::Active(value) => &value.status.manifest,
            Self::Removed(value) => value,
        }
    }

    /// Strict scan order: name, active before removed, then version (bytewise).
    /// Use the returned manifest's release as the next request's exclusive cursor.
    /// The exact closed attempt keeps cursor identity stable across pages.
    #[must_use]
    pub fn order_key(&self) -> (&str, bool, &str) {
        let release = &self.manifest().release;

        (
            &release.name,
            matches!(self, Self::Removed(_)),
            &release.version,
        )
    }
}

/// Versioned preservation page; unlike the historical Page, retains label state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PageV2 {
    /// Unchanged quota/counters across the closed scan, including removed labels.
    pub catalog: Catalog,
    /// One entry in Entry::order_key order, or an explicit end page.
    pub entry: Option<Entry>,
}
