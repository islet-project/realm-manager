use std::{path::Path, process::Command};

pub trait CommandRunner {
    fn get_command(&self) -> &Command;
    fn setup_disk(&self, command: &mut Command, app_disk_path: &Path);
}
