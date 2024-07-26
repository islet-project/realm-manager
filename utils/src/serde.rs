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
    use tokio::{net::UnixStream, task::JoinHandle};

    #[tokio::test]
    async fn communication_through_tokio_serde() {
        let (sender_stream, receiver_stream) = UnixStream::pair().unwrap();
        let mut sender = super::JsonFramed::<UnixStream, u32, u32>::new(sender_stream);
        let mut receiver = super::JsonFramed::<UnixStream, u32, u32>::new(receiver_stream);
        let task: JoinHandle<Result<(), super::JsonFramedError>> = tokio::spawn(async move {
            const MESSAGE: u32 = 0;
            sender.send(MESSAGE).await?;
            let result = sender.recv().await?;
            assert_eq!(result, MESSAGE);
            Ok(())
        });
        let message = receiver.recv().await.unwrap();
        receiver.send(message).await.unwrap();
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn sender_disconnect() {
        let (sender_stream, receiver_stream) = UnixStream::pair().unwrap();
        let mut receiver = super::JsonFramed::<UnixStream, u32, u32>::new(receiver_stream);
        let task: JoinHandle<()> = tokio::spawn(async move {
            let _ = super::JsonFramed::<UnixStream, u32, u32>::new(sender_stream);
        });
        assert!(match receiver.recv().await {
            Err(super::JsonFramedError::StreamIsClosed()) => Ok(()),
            _ => Err(()),
        }
        .is_ok());
        task.await.unwrap()
    }

    #[tokio::test]
    async fn receive_error() {
        let (sender_stream, receiver_stream) = UnixStream::pair().unwrap();
        let mut sender = super::JsonFramed::<UnixStream, String, String>::new(sender_stream);
        let mut receiver = super::JsonFramed::<UnixStream, f32, f32>::new(receiver_stream);
        let task: JoinHandle<Result<(), super::JsonFramedError>> = tokio::spawn(async move {
            sender.send(String::from("")).await?;
            Ok(())
        });
        assert!(match receiver.recv().await {
            Err(super::JsonFramedError::SerdeReadError(_)) => Ok(()),
            _ => Err(()),
        }
        .is_ok());
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn receiver_disconnect() {
        let (sender_stream, receiver_stream) = UnixStream::pair().unwrap();
        let mut sender = super::JsonFramed::<UnixStream, u32, u32>::new(sender_stream);
        let task: JoinHandle<()> = tokio::spawn(async move {
            let _ = super::JsonFramed::<UnixStream, u32, u32>::new(receiver_stream);
        });
        assert!(match sender.send(0).await {
            Err(super::JsonFramedError::SerdeWriteError(_)) => Ok(()),
            _ => Err(()),
        }
        .is_ok());
        task.await.unwrap()
    }
}
