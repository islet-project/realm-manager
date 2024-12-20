use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplicationInfo {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub image_registry: String,
    pub image_part_uuid: Uuid,
    pub data_part_uuid: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    ProvisionInfo(Vec<ApplicationInfo>),
    GetAttestationToken(Vec<u8>),
    GetIfAddrs(),
    CheckStatus(Uuid),
    StartApp(Uuid),
    StopApp(Uuid),
    KillApp(Uuid),
    Reboot(),
    Shutdown(),
}
