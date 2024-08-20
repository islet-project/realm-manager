use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationError {
    #[error("Can't start the application: {0}")]
    ApplicationStart(String),
    #[error("Can't stop the application: {0}")]
    ApplicationStop(String),
    #[error("Can't update the application configuration: {0}")]
    ConfigUpdate(String),
    #[error("Failed to perform operation on application's disk: {0}")]
    DiskOpertaion(String),
}

#[derive(Debug, Error, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ApplicationClientError {}

#[async_trait]
pub trait Application {
    async fn stop(&mut self) -> Result<(), ApplicationError>;
    async fn start(&mut self) -> Result<(), ApplicationError>;
    async fn update_config(&mut self, config: ApplicationConfig) -> Result<(), ApplicationError>;
    async fn prepare_for_next_run(&mut self) -> Result<(), ApplicationError>;
    async fn get_data(&self) -> Result<ApplicationData, ApplicationError>;
}

#[async_trait]
pub trait ApplicationDisk {
    async fn create_disk_with_partitions(&self) -> Result<(), ApplicationError>;
    async fn update_disk_with_partitions(
        &mut self,
        new_data_part_size_mb: u32,
        new_image_part_size_mb: u32,
    ) -> Result<(), ApplicationError>;
    async fn get_data_partition_uuid(&self) -> Result<Uuid, ApplicationError>;
    async fn get_image_partition_uuid(&self) -> Result<Uuid, ApplicationError>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Default)]
pub struct ApplicationConfig {
    pub name: String,
    pub version: String,
    pub image_registry: String,
    pub image_storage_size_mb: u32,
    pub data_storage_size_mb: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct ApplicationData {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub image_registry: String,
    pub image_part_uuid: Uuid,
    pub data_part_uuid: Uuid,
}
