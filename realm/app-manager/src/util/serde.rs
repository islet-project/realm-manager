use futures::SinkExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::{Json, SymmetricalJson};
use tokio_util::codec::{BytesCodec, Framed, LengthDelimitedCodec};
use futures_util::stream::TryStreamExt;

use super::Result;


#[derive(Debug, Error)]
pub enum JsonError {
    #[error("Serde serialization error")]
    SerializationError(#[from] serde_json::Error)
}

pub fn json_dump(obj: impl Serialize + Sized) -> Result<String> {
    Ok(serde_json::to_string(&obj).map_err(JsonError::SerializationError)?)
}

pub fn json_load<T: DeserializeOwned>(content: impl AsRef<str>) -> Result<T> {
    Ok(serde_json::from_str(content.as_ref()).map_err(JsonError::SerializationError)?)
}

