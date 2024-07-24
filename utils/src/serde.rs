use futures::SinkExt;
use futures_util::stream::TryStreamExt;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::formats::Json;
use tokio_util::codec::LengthDelimitedCodec;

#[derive(Debug, Error)]
pub enum JsonFramedError {
    #[error("Serde read error")]
    SerdeReadError(#[source] std::io::Error),

    #[error("Serde write error")]
    SerdeWriteError(#[source] std::io::Error),

    #[error("Stream is closed")]
    StreamIsClosed(),
}

pub struct JsonFramed<
    Transport: AsyncRead + AsyncWrite + Unpin,
    RecvItem: DeserializeOwned + Unpin,
    SendItem: Serialize + Unpin,
> {
    frame: tokio_serde::Framed<
        tokio_util::codec::Framed<Transport, LengthDelimitedCodec>,
        RecvItem,
        SendItem,
        Json<RecvItem, SendItem>,
    >,
}

impl<
        Transport: AsyncRead + AsyncWrite + Unpin,
        RecvItem: DeserializeOwned + Unpin,
        SendItem: Serialize + Unpin,
    > JsonFramed<Transport, RecvItem, SendItem>
{
    pub fn new(stream: Transport) -> Self {
        let length_framed = tokio_util::codec::Framed::new(stream, LengthDelimitedCodec::new());

        Self {
            frame: tokio_serde::Framed::new(length_framed, Json::default()),
        }
    }

    pub async fn recv(&mut self) -> Result<RecvItem, JsonFramedError> {
        self.frame
            .try_next()
            .await
            .map_err(JsonFramedError::SerdeReadError)?
            .ok_or(JsonFramedError::StreamIsClosed())
    }

    pub async fn send(&mut self, v: SendItem) -> Result<(), JsonFramedError> {
        self.frame
            .send(v)
            .await
            .map_err(JsonFramedError::SerdeWriteError)
    }
}

#[cfg(test)]
mod test {
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn communication_through_tokio_serde() {
        let (sock1, sock2) = UnixStream::pair().unwrap();
        let mut communicator1 = super::JsonFramed::<UnixStream, u32, u32>::new(sock1);
        let mut communicator2 = super::JsonFramed::<UnixStream, u32, u32>::new(sock2);
        let task: tokio::task::JoinHandle<Result<(), super::JsonFramedError>> =
            tokio::spawn(async move {
                const MESSAGE: u32 = 0;
                communicator1.send(MESSAGE).await?;
                let result = communicator1.recv().await?;
                assert_eq!(result, MESSAGE);
                Ok(())
            });
        let message = communicator2.recv().await.unwrap();
        communicator2.send(message).await.unwrap();
        assert!(task.await.unwrap().is_ok());
    }
}
