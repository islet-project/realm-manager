use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

use super::Result;

#[derive(Debug, Error)]
pub enum JsonError {
    #[error("Serde serialization error")]
    SerializationError(#[from] serde_json::Error),
}

pub fn json_dump(obj: impl Serialize + Sized) -> Result<String> {
    Ok(serde_json::to_string(&obj).map_err(JsonError::SerializationError)?)
}

pub fn json_load<T: DeserializeOwned>(content: impl AsRef<str>) -> Result<T> {
    Ok(serde_json::from_str(content.as_ref()).map_err(JsonError::SerializationError)?)
}

pub fn json_load_bytes<T: DeserializeOwned>(content: impl AsRef<[u8]>) -> Result<T> {
    Ok(serde_json::from_slice(content.as_ref()).map_err(JsonError::SerializationError)?)
}
