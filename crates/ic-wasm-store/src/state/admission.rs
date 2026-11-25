//! Store owns mutation admission; an external owner still owns installation and quiescence.
use super::{Error, MemoryBacking, State};
use ic_wasm_store_protocol::{Admission, AdmissionCommand};

impl<M: MemoryBacking> State<M> {
    pub fn admission(
        &mut self,
        caller: &[u8],
        is_controller: impl FnOnce(&[u8]) -> bool,
        command: &AdmissionCommand,
    ) -> Result<Option<Admission>, Error> {
        let mut settings = self.settings.get().clone();
        let attempt = match command {
            AdmissionCommand::Observe => return Ok(settings.admission),
            AdmissionCommand::Close(attempt) | AdmissionCommand::Activate(attempt) => attempt,
        };
        if caller.is_empty()
            || caller.len() > 29
            || caller == [4]
            || attempt.sequence == 0
            || attempt.operation == [0; 32]
        {
            return Err(Error::Invalid);
        }
        if matches!(command, AdmissionCommand::Activate(_)) {
            let old = settings.admission.as_mut().ok_or(Error::Conflict)?;
            if old.attempt != *attempt || (old.owner != caller && is_controller(&old.owner)) {
                return Err(Error::Conflict);
            }
            if !old.closed {
                return Ok(settings.admission);
            }
            // Keep the original owner and sequence, including after controller rotation.
            old.closed = false;
        } else {
            let previous = settings.admission.as_ref();
            if previous.is_some_and(|old| old.attempt == *attempt && old.owner == caller) {
                return Ok(settings.admission);
            }
            if previous.is_some_and(|old| old.closed)
                || previous
                    .map_or(0, |old| old.attempt.sequence)
                    .checked_add(1)
                    != Some(attempt.sequence)
            {
                return Err(Error::Conflict);
            }
            settings.read_grants.clear();
            settings.admission = Some(Admission {
                attempt: attempt.clone(),
                owner: caller.to_vec(),
                closed: true,
            });
        }
        let result = settings.admission.clone();
        self.settings.set(settings);

        Ok(result)
    }

    /// A cache continuation must retain this epoch, even if admission reopens before its reply.
    pub fn admission_epoch(&self) -> Result<u64, Error> {
        match &self.settings.get().admission {
            Some(gate) if gate.closed => Err(Error::Closed),
            Some(gate) => Ok(gate.attempt.sequence),
            None => Ok(0),
        }
    }

    pub fn cache_fence(&self, admitted: u64) -> Result<(), Error> {
        if self.admission_epoch()? != admitted {
            return Err(Error::Conflict);
        }

        Ok(())
    }
}
