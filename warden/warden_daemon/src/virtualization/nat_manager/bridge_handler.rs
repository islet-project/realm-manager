use ipnet::IpNet;

use super::devices::Bridge;

pub struct VirtualBridgeHandler;

impl VirtualBridgeHandler {
    pub async fn create_bridge(
        name: String,
        ip: IpNet,
    ) -> Result<Box<dyn Bridge + Send + Sync>, impl ToString> {
        rtnetlink_wrapper::RtNetLinkBridge::new(name, ip)
            .await
            .map(|bridge| {
                let bridge: Box<dyn Bridge + Send + Sync> = Box::new(bridge);
                bridge
            })
    }
    pub async fn delete_bridge(bridge: &(dyn Bridge + Send + Sync)) -> Result<(), impl ToString> {
        rtnetlink_wrapper::RtNetLinkBridge::delete_bridge(bridge.get_name().to_string()).await
    }
}

mod rtnetlink_wrapper {

    use async_trait::async_trait;
    use ipnet::IpNet;
    use rtnetlink::Handle;
    use thiserror::Error;

    use crate::virtualization::nat_manager::{
        devices::{Bridge, BridgeError, Tap},
        utils::rtnetlink_wrapper::{
            get_device_id, get_handler_and_connection, CommonRtNetLinkErrors,
        },
    };

    #[derive(Error, Debug)]
    pub enum RtNetLinkBridgeError {
        #[error("Error occured while using RtNetLink: {0}")]
        RtNetLink(#[source] CommonRtNetLinkErrors),
        #[error("Can't create a bride: {bridge_name} err: {err}")]
        BridgeCreation {
            bridge_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Can't delete a bride: {bridge_name} err: {err}")]
        BridgeDeletion {
            bridge_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Can't assign an ip to a bride: {bridge_name} err: {err}")]
        BridgeIpAssign {
            bridge_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Can't set up bride: {device_name} err: {err}")]
        DeviceIfUp {
            device_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Failed to add interface: {tap_name} to the bridge: {err}")]
        BridgeAddIf {
            tap_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Failed to delete interface: {tap_name} from the bridge: {err}")]
        BridgeDelIf {
            tap_name: String,
            #[source]
            err: rtnetlink::Error,
        },
        #[error("Missing device: {0}")]
        MissingDevice(String),
        #[error("Error while acquiring device id: {0}")]
        GetDeviceId(rtnetlink::Error),
    }

    pub struct RtNetLinkBridge {
        name: String,
    }

    impl RtNetLinkBridge {
        pub async fn new(name: String, ip: IpNet) -> Result<Self, RtNetLinkBridgeError> {
            let bridge = Self { name };
            let (handle, connection) = get_handler_and_connection().map_err(|err| {
                RtNetLinkBridgeError::RtNetLink(CommonRtNetLinkErrors::ConnectionCreation(err))
            })?;

            bridge.create_bridge(&handle).await.map_err(|err| {
                RtNetLinkBridgeError::BridgeCreation {
                    bridge_name: bridge.name.clone(),
                    err,
                }
            })?;

            let bridge_id = get_device_id(&handle, bridge.name.clone())
                .await
                .map_err(RtNetLinkBridgeError::GetDeviceId)?
                .ok_or(RtNetLinkBridgeError::MissingDevice(bridge.name.clone()))?;

            handle
                .address()
                .add(bridge_id, ip.addr(), ip.prefix_len())
                .execute()
                .await
                .map_err(|err| RtNetLinkBridgeError::BridgeIpAssign {
                    bridge_name: bridge.name.clone(),
                    err,
                })?;

            bridge
                .set_interface_up(&handle, bridge_id)
                .await
                .map_err(|err| RtNetLinkBridgeError::DeviceIfUp {
                    device_name: bridge.name.clone(),
                    err,
                })?;

            connection.abort();
            Ok(bridge)
        }

        pub async fn delete_bridge(name: String) -> Result<(), RtNetLinkBridgeError> {
            let (handle, connection) = get_handler_and_connection().map_err(|err| {
                RtNetLinkBridgeError::RtNetLink(CommonRtNetLinkErrors::ConnectionCreation(err))
            })?;
            let bridge_id = get_device_id(&handle, name.clone())
                .await
                .map_err(RtNetLinkBridgeError::GetDeviceId)?
                .ok_or(RtNetLinkBridgeError::MissingDevice(name.clone()))?;

            handle
                .link()
                .del(bridge_id)
                .execute()
                .await
                .map_err(|err| RtNetLinkBridgeError::BridgeDeletion {
                    bridge_name: name,
                    err,
                })?;

            connection.abort();
            Ok(())
        }

        async fn create_bridge(&self, handle: &Handle) -> Result<(), rtnetlink::Error> {
            handle
                .link()
                .add()
                .bridge(self.name.clone())
                .execute()
                .await
        }

        async fn set_interface_up(
            &self,
            handle: &Handle,
            bridge_id: u32,
        ) -> Result<(), rtnetlink::Error> {
            handle.link().set(bridge_id).up().execute().await
        }

        async fn add_tap_device_to_bridge(
            &mut self,
            tap_name: String,
        ) -> Result<(), RtNetLinkBridgeError> {
            let (handle, connection) = get_handler_and_connection().map_err(|err| {
                RtNetLinkBridgeError::RtNetLink(CommonRtNetLinkErrors::ConnectionCreation(err))
            })?;

            let bridge_id = get_device_id(&handle, self.name.clone())
                .await
                .map_err(RtNetLinkBridgeError::GetDeviceId)?
                .ok_or(RtNetLinkBridgeError::MissingDevice(self.name.clone()))?;
            let tap_id = get_device_id(&handle, tap_name.clone())
                .await
                .map_err(RtNetLinkBridgeError::GetDeviceId)?
                .ok_or(RtNetLinkBridgeError::MissingDevice(tap_name.clone()))?;

            handle
                .link()
                .set(tap_id)
                .controller(bridge_id)
                .execute()
                .await
                .map_err(|err| RtNetLinkBridgeError::BridgeAddIf {
                    tap_name: tap_name.clone(),
                    err,
                })?;

            connection.abort();
            Ok(())
        }

        async fn remove_tap_device_from_bridge(
            &mut self,
            tap_name: String,
        ) -> Result<(), RtNetLinkBridgeError> {
            let (handle, connection) = get_handler_and_connection().map_err(|err| {
                RtNetLinkBridgeError::RtNetLink(CommonRtNetLinkErrors::ConnectionCreation(err))
            })?;

            let tap_id = get_device_id(&handle, tap_name.clone())
                .await
                .map_err(RtNetLinkBridgeError::GetDeviceId)?
                .ok_or(RtNetLinkBridgeError::MissingDevice(self.name.clone()))?;
            handle
                .link()
                .set(tap_id)
                .nocontroller()
                .execute()
                .await
                .map_err(|err| RtNetLinkBridgeError::BridgeDelIf { tap_name, err })?;

            connection.abort();
            Ok(())
        }
    }

    #[async_trait]
    impl Bridge for RtNetLinkBridge {
        async fn add_tap_device_to_bridge(
            &mut self,
            tap: &(dyn Tap + Send + Sync),
        ) -> Result<(), BridgeError> {
            Ok(self
                .add_tap_device_to_bridge(tap.get_name().to_string())
                .await
                .map_err(|err| BridgeError::AddTap(err.to_string()))?)
        }
        async fn remove_tap_device_from_bridge(
            &mut self,
            tap: &(dyn Tap + Send + Sync),
        ) -> Result<(), BridgeError> {
            Ok(self
                .remove_tap_device_from_bridge(tap.get_name().to_string())
                .await
                .map_err(|err| BridgeError::RemoveTap(err.to_string()))?)
        }
        fn get_name(&self) -> &str {
            &self.name
        }
    }
}
