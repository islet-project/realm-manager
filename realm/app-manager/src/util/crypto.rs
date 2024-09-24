use p384::ecdsa::{Signature, VerifyingKey};
use p384::pkcs8::DecodePublicKey;
use p384::ecdsa::signature::Verifier;
use thiserror::Error;

use super::Result;


#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Public key parsing error")]
    PubKeyParsing(#[source] p384::pkcs8::spki::Error),

    #[error("Signature parsing error")]
    SignatureParsing(#[source] p384::ecdsa::Error),

    #[error("Verification error")]
    Verification(#[source] p384::ecdsa::Error)
}

#[derive(Debug, Clone, Copy)]
pub struct EcdsaKey {
    key: VerifyingKey
}

impl EcdsaKey {
    pub fn import(der: impl AsRef<[u8]>) -> Result<Self> {
        let key = VerifyingKey::from_public_key_der(der.as_ref())
            .map_err(CryptoError::PubKeyParsing)?;

        Ok(Self { key })
    }

    pub fn verify(&self, msg: impl AsRef<[u8]>, signature: impl AsRef<[u8]>) -> Result<()> {
        let signature = Signature::from_der(signature.as_ref())
            .map_err(CryptoError::SignatureParsing)?;

        self.key.verify(msg.as_ref(), &signature)
            .map_err(CryptoError::Verification)?;

        Ok(())
    }
}


