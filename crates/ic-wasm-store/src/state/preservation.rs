//! Enumerate existing metadata only while its captured owner keeps the exact attempt closed.
use super::{Excluded, Key, MemoryBacking, State, Unbounded};
use ic_wasm_store_protocol::{
    Error,
    preservation::{Catalog, Entry, Page, PageV2, Request},
};

impl<M: MemoryBacking> State<M> {
    pub fn preservation(&self, caller: &[u8], request: &Request) -> Result<Page, Error> {
        let page = self.preservation_v2(caller, request)?;
        Ok(Page {
            catalog: page.catalog,
            entry: page.entry.map(|entry| match entry {
                Entry::Active(custody) => custody,
                Entry::Removed(manifest) => ic_wasm_store_protocol::Custody {
                    status: ic_wasm_store_protocol::Status {
                        published: false,
                        missing: (0..manifest.chunks.len() as u32).collect(),
                        manifest,
                    },
                    pins: vec![],
                },
            }),
        })
    }

    pub fn preservation_v2(&self, caller: &[u8], request: &Request) -> Result<PageV2, Error> {
        let settings = self.settings.get();
        if !settings.admission.as_ref().is_some_and(|gate| {
            gate.closed && gate.owner == caller && gate.attempt == request.attempt
        }) {
            return Err(Error::Conflict);
        }
        if request
            .after
            .as_ref()
            .is_some_and(|release| !release.valid())
        {
            return Err(Error::Invalid);
        }
        let after = request.after.as_ref().map_or(Unbounded, |r| {
            Excluded(if self.releases.contains_key(&Key::tombstone(r)) {
                Key::tombstone(r)
            } else {
                Key::new(r)
            })
        });
        let entry = self
            .releases
            .keys_range((after, Unbounded))
            .next()
            .map(|key| {
                if key.is_tombstone() {
                    Ok(Entry::Removed(self.removed_manifest(&key)))
                } else {
                    self.custody(&key.release()).map(Entry::Active)
                }
            })
            .transpose()?;
        Ok(PageV2 {
            catalog: Catalog {
                quota: settings.quota.clone(),
                reserved_bytes: settings.reserved_bytes,
                releases: self.releases.len(),
                artifacts: self.artifacts.len(),
                chunks: self.chunks.len(),
            },
            entry,
        })
    }
}
