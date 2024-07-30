use std::{io, path::PathBuf};

pub struct WorkspaceManager {
    root_dir: PathBuf,
}

impl WorkspaceManager {
    pub async fn new(path: PathBuf) -> Result<Self, io::Error> {
        if tokio::fs::read_dir(&path).await.is_err() {
            tokio::fs::create_dir(&path).await?;
        }
        Ok(Self { root_dir: path })
    }

    pub async fn create_subdirectory(&self, name: &str) -> Result<(), io::Error> {
        let mut path = self.root_dir.clone();
        path.push(name);
        tokio::fs::create_dir(&path).await
    }

    pub async fn read_subdirectories(&self) -> Result<Vec<PathBuf>, io::Error> {
        let mut subdirectories = vec![];
        let mut read_dir = tokio::fs::read_dir(&self.root_dir).await?;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    subdirectories.push(entry.path());
                }
            }
        }
        Ok(subdirectories)
    }

    pub async fn destroy_workspace(self) -> Result<(), io::Error> {
        tokio::fs::remove_dir_all(&self.root_dir).await
    }
}

#[cfg(test)]
mod test {
    use std::{ffi::OsString, path::PathBuf};

    const FILE_PATH: &str = "/tmp/realm_manager_fs_repository_test_dir";

    impl Drop for super::WorkspaceManager {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(FILE_PATH);
        }
    }

    #[tokio::test]
    async fn create_workspace_manager() {
        const SUBDIRECTORY: &str = "subdir";
        let workspace_manager = super::WorkspaceManager::new(PathBuf::from(FILE_PATH))
            .await
            .unwrap();
        workspace_manager
            .create_subdirectory(SUBDIRECTORY)
            .await
            .unwrap();
        let subdirectories: Vec<OsString> = workspace_manager
            .read_subdirectories()
            .await
            .unwrap()
            .into_iter()
            .map(|path_buf| path_buf.file_name().unwrap().to_owned())
            .filter(|path_buf| path_buf.to_str().unwrap() == SUBDIRECTORY)
            .collect();
        assert_eq!(subdirectories.len(), 1);
    }
}
