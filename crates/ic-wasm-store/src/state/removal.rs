//! Exact release removal; retained identities and shared payloads have separate lifetimes.
use super::{Error, Excluded, Key, Manifest, MemoryBacking, State, Unbounded};
use ic_wasm_store_protocol::{PAGE_SIZE, Release};
use std::collections::BTreeSet;

impl<M: MemoryBacking> State<M> {
    pub(super) fn active_hashes(&self) -> BTreeSet<[u8; 32]> {
        self.releases
            .iter()
            .filter(|entry| !entry.key().is_tombstone())
            .map(|entry| entry.value())
            .collect()
    }

    pub(super) fn removed_manifest(&self, key: &Key) -> Manifest {
        let hash = self.releases.get(key).expect("removed release");
        let artifact = self.artifacts.get(&hash).expect("retained identity");
        Manifest {
            release: key.release(),
            hash,
            bytes: artifact.bytes,
            chunks: artifact.chunks,
        }
    }

    pub fn tombstones(&self, after: Option<&Release>) -> Result<Vec<Manifest>, Error> {
        if after.is_some_and(|release| !release.valid()) {
            return Err(Error::Invalid);
        }
        let lower = after.map_or(Unbounded, |release| Excluded(Key::tombstone(release)));
        Ok(self
            .releases
            .range((lower, Unbounded))
            .map(|entry| entry.key().clone())
            .filter(Key::is_tombstone)
            .take(PAGE_SIZE)
            .map(|key| self.removed_manifest(&key))
            .collect())
    }

    pub fn remove_release(
        &mut self,
        manifest: &Manifest,
        published: bool,
    ) -> Result<Manifest, Error> {
        self.admission_epoch()?;
        if !manifest.valid() {
            return Err(Error::Invalid);
        }
        let key = Key::new(&manifest.release);
        let tombstone = Key::tombstone(&manifest.release);
        if self.releases.contains_key(&tombstone) {
            let old = self.removed_manifest(&tombstone);
            return if old == *manifest {
                Ok(old)
            } else {
                Err(Error::Conflict)
            };
        }
        let status = self.status(&manifest.release)?;
        if status.manifest != *manifest || status.published != published {
            return Err(Error::Conflict);
        }
        let mut artifact = self
            .artifacts
            .get(&manifest.hash)
            .expect("retained artifact");
        // Protect all aliases of pinned artifacts, including outstanding cache work.
        if !artifact.pins.is_empty() {
            return Err(Error::Conflict);
        }
        let shared = self.releases.iter().any(|entry| {
            !entry.key().is_tombstone() && entry.key() != &key && entry.value() == manifest.hash
        });
        let mut unused: BTreeSet<_> = artifact.chunks.iter().copied().collect();
        if !shared {
            for hash in self.active_hashes() {
                if hash != manifest.hash {
                    for chunk in self.artifacts.get(&hash).expect("active artifact").chunks {
                        unused.remove(&chunk);
                    }
                }
            }
        }
        self.releases.insert(tombstone, manifest.hash);
        self.releases.remove(&key);
        if !shared {
            // Keep geometry and full digest even after the last active alias disappears.
            artifact.published = false;
            let bytes = artifact.bytes;
            self.artifacts.insert(manifest.hash, artifact);
            for hash in unused {
                self.chunks.remove(&hash);
            }
            let mut settings = self.settings.get().clone();
            settings.reserved_bytes = settings
                .reserved_bytes
                .checked_sub(bytes)
                .expect("artifact reservation");
            self.settings.set(settings);
        }
        Ok(manifest.clone())
    }
}
