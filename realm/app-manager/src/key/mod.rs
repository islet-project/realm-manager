use thiserror::Error;

use ring::KeyRingError;

pub mod dummy;
pub mod ring;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("Key ring error")]
    KeyRingError(#[from] KeyRingError),
}

pub type Result<T> = std::result::Result<T, KeyError>;

pub trait KeySealing {
    fn derive_key(&self, infos: &mut dyn Iterator<Item = &&[u8]>) -> Result<Vec<u8>>;
    fn seal(
        self: Box<Self>,
        infos: &mut dyn Iterator<Item = &&[u8]>,
    ) -> Result<Box<dyn KeySealing + Send + Sync>>;
}
