use std::array::TryFromSliceError;

use ratls::InternalTokenResolver;
use ratls::RaTlsError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RsiTokenResolverError {
    #[error("Failed to read attestation token from RMM")]
    RsiCallFailed(#[from] rust_rsi::NixError),

    #[error("Challenge must be 64 bytes long")]
    InvalidChallengeLength(#[from] TryFromSliceError),
}

#[derive(Debug)]
pub struct RsiTokenResolver;

impl RsiTokenResolver {
    pub fn new() -> Self {
        Self {}
    }
}

fn ratls_error<E: Into<RsiTokenResolverError>>(e: E) -> RaTlsError {
    let rsi_error: RsiTokenResolverError = e.into();
    RaTlsError::GenericTokenResolverError(Box::new(rsi_error))
}

impl InternalTokenResolver for RsiTokenResolver {
    fn resolve(&self, challenge: &[u8]) -> std::result::Result<Vec<u8>, RaTlsError> {
        let challenge: [u8; 64] = challenge.try_into().map_err(ratls_error)?;

        rust_rsi::attestation_token(&challenge).map_err(ratls_error)
    }
}
