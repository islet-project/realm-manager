use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct ApplicationConfig {
    pub name: String,
    pub version: String,
    pub image_registry: String,
    pub image_storage_size_mb: u32,
    pub data_storage_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct ApplicationDescription {
    pub uuid: Uuid,
}
