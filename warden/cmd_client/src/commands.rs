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

        /// Path to kernel's initramfs
        #[clap(short = 'i', long, default_value = None)]
        kernel_initramfs: Option<PathBuf>,

        /// Additional kernel options
        #[clap(short = 'o', long, default_value = None)]
        kernel_options: Option<String>,

        /// Path to kernel image
        #[clap(short = 'k', long)]
        kernel: PathBuf,

        /// VSOCK cid for realm
        #[clap(short = 'v', long)]
        vsock_cid: u32,
    },

    ListRealms,

    StartRealm {
        /// Realm Id
        #[clap(short = 'r', long)]
        id: String,
    },

    InspectRealm {
        /// Realm Id
        #[clap(short = 'r', long)]
        id: String,
    },

    StopRealm {
        /// Realm Id
        #[clap(short = 'r', long)]
        id: String,
    },

    RebootRealm {
        /// Realm Id
        #[clap(short = 'r', long)]
        id: String,
    },

    DestroyRealm {
        /// Realm Id
        #[clap(short = 'r', long)]
        id: String,
    },

    CreateApplication {
        /// Realm Id in which application will be created
        #[clap(short, long)]
        realm_id: String,

        /// Application name
        #[clap(short, long)]
        name: String,

        /// Application version
        #[clap(short, long)]
        version: String,

        /// Application image registry
        #[clap(short, long)]
        image_registry: String,

        /// Application image storage size
        #[clap(short = 'o', long)]
        image_storage_size_mb: u32,

        /// Application data storage size
        #[clap(short, long)]
        data_storage_size_mb: u32,
    },

    StartApplication {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,
    },

    StopApplication {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,
    },

    UpdateApplication {
        /// Application Id
        #[clap(short, long)]
        application_id: String,

        /// Realm Id
        #[clap(short, long)]
        realm_id: String,

        /// Application name
        #[clap(short, long)]
        name: String,

        /// Application version
        #[clap(short, long)]
        version: String,

        /// Application image registry
        #[clap(short, long)]
        image_registry: String,

        /// Application image storage size
        #[clap(short = 'o', long)]
        image_storage_size_mb: u32,

        /// Application data storage size
        #[clap(short, long)]
        data_storage_size_mb: u32,
    },
}
