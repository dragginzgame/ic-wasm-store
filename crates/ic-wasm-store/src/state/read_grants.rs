//! Exact read authority retained until expiry or the next maintenance closure.

use super::{Error, MemoryBacking, State, records::RetainedReadGrant};
use ic_wasm_store_protocol::{
    Chunk, GrantedRead, MAX_READ_GRANT_NS, MAX_READ_GRANTS, Read, ReadGrant, digest,
};

impl<M: MemoryBacking> State<M> {
    pub fn grant_read(&mut self, owner: &[u8], now: u64, grant: &ReadGrant) -> Result<(), Error> {
        if grant.epoch != self.admission_epoch()? {
            return Err(Error::Conflict);
        }
        if grant.installer.is_empty()
            || grant.installer.len() > 29
            || grant.installer == [4]
            || grant.expires_at_ns <= now
            || grant.expires_at_ns - now > MAX_READ_GRANT_NS
        {
            return Err(Error::Invalid);
        }
        self.observe_pin(owner, &grant.pin)?;

        let fingerprint = fingerprint(owner, grant)?;
        let mut settings = self.settings.get().clone();
        settings
            .read_grants
            .retain(|retained| retained.expires_at_ns > now);
        if settings
            .read_grants
            .iter()
            .any(|retained| retained.fingerprint == fingerprint)
        {
            return Ok(());
        }
        if settings.read_grants.len() == MAX_READ_GRANTS {
            return Err(Error::Capacity);
        }
        settings.read_grants.push(RetainedReadGrant {
            fingerprint,
            installer: grant.installer.clone(),
            issuer: owner.to_vec(),
            expires_at_ns: grant.expires_at_ns,
        });
        self.settings.set(settings);
        Ok(())
    }

    pub fn has_installer_grant(
        &self,
        installer: &[u8],
        now: u64,
        is_controller: impl Fn(&[u8]) -> bool,
    ) -> bool {
        self.settings.get().read_grants.iter().any(|grant| {
            grant.installer == installer
                && now < grant.expires_at_ns
                && is_controller(&grant.issuer)
        })
    }

    pub fn read_granted(
        &self,
        installer: &[u8],
        now: u64,
        request: &GrantedRead,
    ) -> Result<Chunk, Error> {
        if installer != request.grant.installer
            || now >= request.grant.expires_at_ns
            || self.admission_epoch().ok() != Some(request.grant.epoch)
        {
            return Err(Error::NotFound);
        }
        let fingerprint = fingerprint(&request.owner, &request.grant)?;
        if !self
            .settings
            .get()
            .read_grants
            .iter()
            .any(|retained| retained.fingerprint == fingerprint && retained.expires_at_ns > now)
        {
            return Err(Error::NotFound);
        }
        self.observe_pin(&request.owner, &request.grant.pin)?;
        self.read(&Read {
            release: request.grant.pin.manifest.release.clone(),
            index: request.index,
        })
    }
}

fn fingerprint(owner: &[u8], grant: &ReadGrant) -> Result<[u8; 32], Error> {
    let bytes = postcard::to_allocvec(&(b"Store read grant v1", owner, grant))
        .map_err(|_| Error::Invalid)?;
    Ok(digest(&bytes))
}
