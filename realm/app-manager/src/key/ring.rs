use std::borrow::Borrow;

use keyutils::{keytypes::Logon, KeyType, Keyring, SpecialKeyring};
use thiserror::Error;

use super::Result;

#[derive(Debug, Error)]
pub enum KeyRingError {
    #[error("Failed to attach to requested kernel keyring")]
    FailedToAttachtoKeyring(#[source] keyutils::Error),

    #[error("Error sealing key")]
    FailedToSealKey(#[source] keyutils::Error)
}

pub struct KernelKeyring {
    ring: Keyring
}

impl KernelKeyring {
    pub fn new(parent_ring: SpecialKeyring) -> Result<Self> {
        Ok(Self {
            ring: Keyring::attach_or_create(parent_ring).map_err(KeyRingError::FailedToAttachtoKeyring)?
        })
    }

    pub fn logon_seal(&mut self, subtype: impl AsRef<str>, description: impl AsRef<str>, payload: impl AsRef<[u8]>) -> Result<()> {
        let key_desc = keyutils::keytypes::logon::Description {
            subtype: std::borrow::Cow::Owned(subtype.as_ref().to_owned()),
            description: std::borrow::Cow::Owned(description.as_ref().to_owned())
        };

        let _ = self.ring.add_key::<Logon, _, _>(key_desc, payload.as_ref())
            .map_err(KeyRingError::FailedToSealKey)?;

        Ok(())
    }
}
