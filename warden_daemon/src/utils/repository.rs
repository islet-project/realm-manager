use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, PartialOrd, Clone, Serialize, Deserialize)]
pub enum RepositoryError {
    #[error("Can't create a repository: {0}")]
    CreationFail(String),
    #[error("Can't save inner value: {0}")]
    SaveFail(String),
}

#[async_trait]
pub trait Repository {
    type Data;

    fn get(&self) -> &Self::Data;
    async fn save(&mut self) -> Result<(), RepositoryError>;
}
