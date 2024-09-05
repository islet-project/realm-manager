use sha2::{Digest, Sha256};

use super::{KeySealing, KeySealingFactory, Result};

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
    fn derive_key(&self, infos: &[&[u8]]) -> Result<Vec<u8>> {
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
        infos: &[&[u8]],
        _: &[u8],
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

pub struct DummyKeySealingFactory {
    ikm: Vec<u8>,
}

impl DummyKeySealingFactory {
    pub fn new(ikm: Vec<u8>) -> Self {
        Self { ikm }
    }
}

impl KeySealingFactory for DummyKeySealingFactory {
    fn create(&self) -> Box<dyn KeySealing + Send + Sync> {
        Box::new(DummyKeySealing::new(self.ikm.clone()))
    }
}
