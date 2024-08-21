use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

#[derive(Debug, Error)]
pub enum FileRepositoryError {
    #[error("Error occured: {0}")]
    CreationFail(#[source] std::io::Error),
    #[error("Failed to save file: {0}")]
    SaveFail(String),
    #[error("Failed to read file: {0}")]
    ReadFail(String),
}

pub struct FileRepository<Struct: Serialize + DeserializeOwned> {
    data: Struct,
    path: PathBuf,
}

impl<Struct: Serialize + DeserializeOwned> FileRepository<Struct> {
    pub async fn new(data: Struct, path: &Path) -> Result<Self, FileRepositoryError> {
        let mut repository = Self { data, path: path.to_path_buf() };
        repository.save().await?;
        Ok(repository)
    }

    pub async fn from_file_path(path: &Path) -> Result<Self, FileRepositoryError> {
        FileRepository::<Struct>::try_read_file(path)
            .await
            .map(|(_, data)| Self { data, path: path.to_path_buf() })
    }

    pub async fn save(&mut self) -> Result<(), FileRepositoryError> {
        let yaml_data = serde_yaml::to_string(&self.data)
            .map_err(|err| FileRepositoryError::SaveFail(err.to_string()))?;
        let mut file = File::create(&self.path)
        .await
        .map_err(FileRepositoryError::CreationFail)?;
        file
            .write_all(yaml_data.as_bytes())
            .await
            .map_err(|err| FileRepositoryError::SaveFail(err.to_string()))?;
        file.flush().await.map_err(|err| FileRepositoryError::SaveFail(err.to_string()))?;
        Ok(())
    }

    pub fn get_mut(&mut self) -> &mut Struct {
        &mut self.data
    }

    pub fn get(&self) -> &Struct {
        &self.data
    }

    async fn try_read_file(path: &Path) -> Result<(File, Struct), FileRepositoryError> {
        match File::open(path).await {
            Ok(mut file) => {
                let mut buf = String::new();
                let _ = file
                    .read_to_string(&mut buf)
                    .await
                    .map_err(|err| FileRepositoryError::ReadFail(err.to_string()))?;
                let data: Struct = serde_yaml::from_str(&buf)
                    .map_err(|err| FileRepositoryError::ReadFail(err.to_string()))?;
                Ok((file, data))
            }
            Err(err) => Err(FileRepositoryError::ReadFail(err.to_string())),
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use serde::{de::DeserializeOwned, Serialize};

    use super::FileRepository;

    const FILE_PATH: &str = "/tmp/realm_manager_fs_repository_test_file";

    impl<Struct: Serialize + DeserializeOwned> Drop for FileRepository<Struct> {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(FILE_PATH);
        }
    }

    #[tokio::test]
    async fn create_file_repository() {
        const DATA: u32 = 0;
        let path = PathBuf::from(FILE_PATH);
        let mut file_repository = super::FileRepository::<u32>::new(DATA, &path)
            .await
            .unwrap();
        assert_eq!(*file_repository.get(), DATA);
        let data = file_repository.get_mut();
        *data += 1;
        file_repository.save().await.unwrap();
        file_repository = super::FileRepository::from_file_path(&path).await.unwrap();
        assert_eq!(*file_repository.get(), DATA + 1);
    }
}
