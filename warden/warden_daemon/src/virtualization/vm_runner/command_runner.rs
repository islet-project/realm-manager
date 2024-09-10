use std::process::Command;

use uuid::Uuid;

pub trait CommandRunner {
    fn get_command(&self) -> &Command;
    fn setup_disk(&self, command: &mut Command, application_uuids: &[&Uuid]);
}
