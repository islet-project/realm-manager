pub mod dummy;
pub mod hkdf;
pub mod ring;

use crate::error::Result;

pub trait KeySealing {
    fn derive_key(&self, infos: &[&[u8]]) -> Result<Vec<u8>>;
    fn seal(
        self: Box<Self>,
        infos: &[&[u8]],
        image_hash: &[u8],
    ) -> Result<Box<dyn KeySealing + Send + Sync>>;
}

pub trait KeySealingFactory {
    fn create(&self) -> Box<dyn KeySealing + Send + Sync>;
}
