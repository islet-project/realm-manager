use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand, Debug, Clone, PartialEq, PartialOrd)]
pub enum Command {
    CreateRealm {
        /// CPU type
        #[clap(short = 'c', long, default_value = "cortex-a57")]
        cpu: String,

        /// Machine type
        #[clap(short = 'm', long, default_value = "virt")]
        machine: String,

        /// CPU core count for realm
        #[clap(short = 'n', long, default_value_t = 2)]
        core_count: usize,

        /// RAM size
        #[clap(short = 'r', long, default_value_t = 2048)]
        ram_size: usize,

        /// TAP device to enable TCP/IP networking
        #[clap(short = 't', long, default_value = "tap100")]
        tap_device: String,

        /// MAC address for realm's network card
        #[clap(short = 'a', long, default_value = "52:55:00:d1:55:01")]
        mac_address: String,

        /// Emulated Network device
        #[clap(short = 'e', long, default_value = "e1000")]
        network_device: Option<String>,

        /// Remote terminal uri
        #[clap(short = 'u', long, default_value = None)]
        remote_terminal_uri: Option<String>,

        /// VSOCK cid for realm
        #[clap(short = 'v', long)]
        vsock_cid: u32,

        /// Path to kernel image
        #[clap(short = 'k', long)]
        kernel: PathBuf,
    },

    ListRealms,

    StartRealm {
        /// Realm Id
        #[clap(short, long)]
        id: String,
    },

    InspectRealm {
        /// Realm Id
        #[clap(short, long)]
        id: String,
    },

    StopRealm {
        /// Realm Id
        #[clap(short, long)]
        id: String,
    },

    DestroyRealm {
        /// Realm Id
        #[clap(short, long)]
        id: String,
    },

    CreateApplication {
        /// Realm Id in which application will be created
        #[clap(short, long)]
        realm_id: String,
    },

    StartApp {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,
    },

    StopApp {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,
    },

    UpdateApp {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,
    },
}
