use sha2::digest::typenum::Unsigned;
use hkdf::Hkdf;
use sha2::{digest::OutputSizeUser, Sha256};
use thiserror::Error;

use crate::{config::IkmSource, consts::APPLICATION_SLK_SALT};
use super::Result;

use super::{KeySealing, KeySealingFactory};

#[derive(Debug, Error)]
pub enum HkdfSealingError {
    #[error("Failed to decode hex in stubbed ikm")]
    StubbedIkmDecodingError(#[source] const_hex::FromHexError)
}

pub struct HkdfSealing {
    ikm: Vec<u8>,
    use_image_hash: bool
}

impl KeySealing for HkdfSealing {
    fn derive_key(&self, infos: &[&[u8]]) -> crate::error::Result<Vec<u8>> {
        let hkdf = Hkdf::<Sha256>::new(None, &self.ikm);
        let mut okm = [0u8; <Sha256 as OutputSizeUser>::OutputSize::USIZE];

        hkdf.expand_multi_info(infos, &mut okm)
            .expect("OKM size mismatch");

        Ok(okm.to_vec())
    }

    fn seal(
            self: Box<Self>,
            infos: &[&[u8]],
            image_hash: &[u8]
        ) -> crate::error::Result<Box<dyn KeySealing + Send + Sync>> {
        let hkdf = Hkdf::<Sha256>::new(Some(&APPLICATION_SLK_SALT), &self.ikm);
        let mut info = infos.concat();

        if self.use_image_hash {
            info.extend(image_hash);
        }

        let mut okm = [0u8; <Sha256 as OutputSizeUser>::OutputSize::USIZE];
        hkdf.expand(&info, &mut okm)
            .expect("OKM size mismatch");

        Ok(Box::new(HkdfSealing {
            ikm: okm.to_vec(),
            use_image_hash: self.use_image_hash
        }))
    }
}

pub struct HkdfSealingFactory {
    ikm: Vec<u8>,
    use_image_hash: bool
}

impl HkdfSealingFactory {
    pub fn new(ikm_source: &IkmSource) -> Result<Self> {
        let ikm = match ikm_source {
            IkmSource::StubbedHex(hex) => const_hex::decode(hex)
                .map_err(HkdfSealingError::StubbedIkmDecodingError)?,
            _ => todo!()
        };

        let use_image_hash = match ikm_source {
            IkmSource::StubbedHex(_) => false,
            _ => todo!()
        };

        Ok(Self {
            ikm,
            use_image_hash
        })
    }
}

impl KeySealingFactory for HkdfSealingFactory {
    fn create(&self) -> Box<dyn KeySealing + Send + Sync> {
        Box::new(HkdfSealing {
            ikm: self.ikm.clone(),
            use_image_hash: self.use_image_hash
        })
    }
}

