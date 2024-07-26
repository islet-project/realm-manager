use std::{io, path::PathBuf};

use crate::managers::{realm::RealmCreator, warden::Warden};

pub struct WardenFabric;

impl WardenFabric {
    pub fn create_warden(realm_creator: Box<dyn RealmCreator + Send + Sync>, warden_workdir_path: PathBuf) -> Result<Box<dyn Warden + Send + Sync>, io::Error> {
        todo!()
    }
}