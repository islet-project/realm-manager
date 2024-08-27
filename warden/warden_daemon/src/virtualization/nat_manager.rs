use std::{collections::HashMap, io};

use async_trait::async_trait;
use tokio::process::Command;
use tokio_tun::TunBuilder;
use uuid::Uuid;

use super::network_manager::{NetworkManager, NetworkManagerError};

pub struct NatManager {
    taps: HashMap<Uuid, String>,
    bridge: String,
}

impl NatManager {
    pub fn new() -> Self {
        Self {
            taps: HashMap::new(),
            bridge: "virbr".to_string(),
        }
    }
}

impl Drop for NatManager {
    fn drop(&mut self) {
        let res: Result<(), io::Error> = self
            .taps
            .iter()
            .try_for_each(|(_, tap): (&Uuid, &String)| {
                let mut cmd = Command::new("brctl");
                cmd.arg("delif").arg(&self.bridge).arg(tap);
                cmd.spawn().map(|_| ())
            });
        if let Err(err) = res {
            log::error!("Failed to detach interface from the bridge: {err}!")
        }
    }
}

#[async_trait]
impl NetworkManager for NatManager {
    async fn create_tap_device_for_realm(
        &mut self,
        tap_name: String,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        if self.taps.contains_key(&realm_id) {
            return Err(NetworkManagerError::TapCreation {
                tap_name,
                err_message: "already exists.".to_string(),
            });
        }

        if let Err(err) = TunBuilder::new()
            .name(&tap_name)
            .persist()
            .up()
            .tap(true)
            .try_build()
        {
            Err(NetworkManagerError::TapCreation {
                tap_name,
                err_message: err.to_string(),
            })
        } else {
            let mut bridge_add_cmd = Command::new("brctl");
            bridge_add_cmd.arg("addif").arg(&self.bridge).arg(&tap_name);
            let res = bridge_add_cmd
                .spawn()
                .map_err(|err| NetworkManagerError::BridgeOperation(err.to_string()))?
                .wait()
                .await
                .map_err(|err| NetworkManagerError::BridgeOperation(err.to_string()))?;
            if !res.success() {
                Err(NetworkManagerError::BridgeAddIf {
                    tap_name,
                    bridge_name: self.bridge.clone(),
                })
            } else {
                self.taps.insert(realm_id, tap_name);
                Ok(())
            }
        }
    }
    async fn prepare_network(&self) -> Result<(), NetworkManagerError> {
        Ok(())
    }

    async fn shutdown_tap_device_for_realm(
        &mut self,
        realm_id: Uuid,
    ) -> Result<(), NetworkManagerError> {
        let tap_name = self
            .taps
            .get(&realm_id)
            .ok_or(NetworkManagerError::MissingTap {
                realm_uuid: realm_id,
            })?;

        let mut cmd = Command::new("brctl");
        cmd.arg("delif").arg(&self.bridge).arg(tap_name);
        let mut res = cmd
            .spawn()
            .map_err(|err| NetworkManagerError::BridgeOperation(err.to_string()))?;
        if !res
            .wait()
            .await
            .map_err(|err| NetworkManagerError::BridgeOperation(err.to_string()))?
            .success()
        {
            Err(NetworkManagerError::BridgeDelIf {
                tap_name: tap_name.clone(),
                bridge_name: self.bridge.clone(),
            })
        } else {
            self.taps.remove(&realm_id);
            Ok(())
        }
    }
}
