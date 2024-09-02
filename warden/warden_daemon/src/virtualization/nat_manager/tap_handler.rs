use std::io;

use super::devices::Tap;

pub struct TapDeviceFabric;

impl TapDeviceFabric {
    pub async fn create_tap(name: String) -> Result<Box<dyn Tap + Send + Sync>, io::Error> {
        tokio_tun_wrapper::create_tap(name).map_err(|err| io::Error::other(err.to_string()))
    }
    pub async fn delete_tap(tap: Box<dyn Tap + Send + Sync>) -> Result<(), io::Error> {
        rtnetlink_wrapper::delete_tap(tap.get_name().to_string())
            .await
            .map_err(|err| io::Error::other(err.to_string()))
    }
}

struct TapDevice {
    name: String,
}

impl Tap for TapDevice {
    fn get_name(&self) -> &str {
        &self.name
    }
}

mod tokio_tun_wrapper {
    use tokio::task::block_in_place;
    use tokio_tun::TunBuilder;

    use crate::virtualization::nat_manager::devices::Tap;

    use super::TapDevice;
    pub fn create_tap(name: String) -> Result<Box<dyn Tap + Send + Sync>, tokio_tun::Error> {
        block_in_place(|| {
            TunBuilder::new()
                .name(&name)
                .persist()
                .up()
                .tap(true)
                .try_build()
        })?;
        Ok(Box::new(TapDevice { name }))
    }
}

mod rtnetlink_wrapper {
    use thiserror::Error;

    use crate::virtualization::nat_manager::utils::rtnetlink_wrapper::{
        get_device_id, get_handler_and_connection, CommonRtNetLinkErrors,
    };

    #[derive(Error, Debug)]
    pub enum RtNetLinkTapError {
        #[error("Error occured while using RtNetLink: {0}")]
        Connection(#[source] CommonRtNetLinkErrors),
        #[error("Error occured while acquiring tap device id: {0}")]
        TapIdAcquire(#[source] rtnetlink::Error),
        #[error("Error occured while deleting tap device: {0}")]
        TapDelete(#[source] rtnetlink::Error),
    }

    pub async fn delete_tap(name: String) -> Result<(), RtNetLinkTapError> {
        let (handle, connection) = get_handler_and_connection().map_err(|err| {
            RtNetLinkTapError::Connection(CommonRtNetLinkErrors::ConnectionCreation(err))
        })?;

        if let Some(id) = get_device_id(&handle, name)
            .await
            .map_err(RtNetLinkTapError::TapIdAcquire)?
        {
            handle
                .link()
                .del(id)
                .execute()
                .await
                .map_err(RtNetLinkTapError::TapDelete)?;
        }

        connection.abort();
        Ok(())
    }
}
