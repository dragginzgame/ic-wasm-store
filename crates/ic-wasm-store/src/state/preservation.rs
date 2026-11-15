//! Enumerate existing metadata only while its captured owner keeps the exact attempt closed.
use super::{Excluded, Key, MemoryBacking, State, Unbounded};
use ic_wasm_store_protocol::{
    Error,
    preservation::{Catalog, Page, Request},
};

impl<M: MemoryBacking> State<M> {
    pub fn preservation(&self, caller: &[u8], request: &Request) -> Result<Page, Error> {
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
                    let manifest = self.removed_manifest(&key);
                    Ok(ic_wasm_store_protocol::Custody {
                        status: ic_wasm_store_protocol::Status {
                            published: false,
                            missing: (0..manifest.chunks.len() as u32).collect(),
                            manifest,
                        },
                        pins: vec![],
                    })
                } else {
                    self.custody(&key.release())
                }
            })
            .transpose()?;
        Ok(Page {
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
