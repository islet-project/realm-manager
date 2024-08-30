pub mod rtnetlink_wrapper {
    use std::io;

    use futures::TryStreamExt;
    use rtnetlink::{new_connection, Handle};
    use thiserror::Error;
    use tokio::task::JoinHandle;

    #[derive(Error, Debug)]
    pub enum CommonRtNetLinkErrors {
        #[error("Failed establish connection: {0}")]
        ConnectionCreation(#[source] io::Error),
    }

    pub fn get_handler_and_connection() -> Result<(Handle, JoinHandle<()>), io::Error> {
        let (connection, handle, _) = new_connection()?;
        let join_handle = tokio::spawn(connection);
        Ok((handle, join_handle))
    }

    pub async fn get_device_id(
        handle: &Handle,
        dev_name: String,
    ) -> Result<Option<u32>, rtnetlink::Error> {
        Ok(handle
            .link()
            .get()
            .match_name(dev_name)
            .execute()
            .try_next()
            .await?
            .map(|val| val.header.index))
    }
}
