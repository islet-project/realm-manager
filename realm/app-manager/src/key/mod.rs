pub mod dummy;
pub mod ring;

use crate::error::Result;

pub trait KeySealing {
    fn derive_key(&self, infos: &mut dyn Iterator<Item = &&[u8]>) -> Result<Vec<u8>>;
    fn seal(
        self: Box<Self>,
        infos: &mut dyn Iterator<Item = &&[u8]>,
    ) -> Result<Box<dyn KeySealing + Send + Sync>>;
}
