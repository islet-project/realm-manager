use futures::SinkExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::{Json, SymmetricalJson};
use tokio_util::codec::{BytesCodec, Framed, LengthDelimitedCodec};
use futures_util::stream::TryStreamExt;

use super::Result;


#[derive(Debug, Error)]
pub enum JsonFramedError {
    #[error("Serde read error")]
    SerdeReadError(#[source] std::io::Error),

    #[error("Serde write error")]
    SerdeWriteError(#[source] std::io::Error),

    #[error("Stream is closed")]
    StreamIsClosed(),

    #[error("Serde serialization error")]
    SerializationError(#[from] serde_json::Error)
}

pub fn json_dump(obj: impl Serialize + Sized) -> Result<String> {
    Ok(serde_json::to_string(&obj).map_err(JsonFramedError::SerializationError)?)
}

pub fn json_load<T: DeserializeOwned>(content: impl AsRef<str>) -> Result<T> {
    Ok(serde_json::from_str(content.as_ref()).map_err(JsonFramedError::SerializationError)?)
}

pub struct JsonFramed<Transport: AsyncRead + AsyncWrite + Unpin, RecvItem: DeserializeOwned + Unpin, SendItem: Serialize + Unpin> {
    frame: tokio_serde::Framed<
        tokio_util::codec::Framed<Transport, LengthDelimitedCodec>,
        RecvItem,
        SendItem,
        Json<RecvItem, SendItem>
    >
}

impl<Transport: AsyncRead + AsyncWrite + Unpin, RecvItem: DeserializeOwned + Unpin, SendItem: Serialize + Unpin> JsonFramed<Transport, RecvItem, SendItem> {
    pub fn new(stream: Transport) -> Self {
        let length_framed = tokio_util::codec::Framed::new(stream, LengthDelimitedCodec::new());

        Self {
            frame: tokio_serde::Framed::new(length_framed, Json::default())
        }
    }

    pub async fn recv(&mut self) -> Result<RecvItem> {
        Ok(
            self.frame.try_next()
                .await
                .map_err(JsonFramedError::SerdeReadError)?
                .ok_or(JsonFramedError::StreamIsClosed())?
        )
    }

    pub async fn send(&mut self, v: SendItem) -> Result<()> {
        Ok(
            self.frame.send(v)
                .await
                .map_err(JsonFramedError::SerdeWriteError)?
        )
    }
}
