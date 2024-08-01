use sha2::{Digest, Sha256};

use super::{KeySealing, Result};

pub struct DummyKeySealing {
    ikm: Vec<u8>,
}

impl DummyKeySealing {
    pub fn new(ikm: impl AsRef<[u8]>) -> Self {
        Self {
            ikm: ikm.as_ref().to_owned(),
        }
    }
}

impl KeySealing for DummyKeySealing {
    fn derive_key(&self, infos: &mut dyn Iterator<Item = &&[u8]>) -> Result<Vec<u8>> {
        let mut hasher = Sha256::new();

        hasher.update(self.ikm.as_slice());
        for info in infos {
            hasher.update(info);
        }

        let digest = hasher.finalize();

        Ok(digest.to_vec())
    }

    fn seal(
        self: Box<Self>,
        infos: &mut dyn Iterator<Item = &&[u8]>,
    ) -> Result<Box<dyn KeySealing + Send + Sync>> {
        let mut hasher = Sha256::new();

        hasher.update(self.ikm.as_slice());
        for info in infos {
            hasher.update(info);
        }

        let digest = hasher.finalize();

        Ok(Box::new(DummyKeySealing::new(digest)))
    }
}
