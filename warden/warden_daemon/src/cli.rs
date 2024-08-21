use std::path::PathBuf;

use clap::Parser;
use tokio_vsock::VMADDR_CID_HOST;

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, value_parser=clap::value_parser!(u32).range(2..), default_value_t = VMADDR_CID_HOST)]
    pub cid: u32,
    #[arg(short, long, value_parser=clap::value_parser!(u32).range(80..), default_value_t = 80)]
    pub port: u32,
    #[arg(short, long)]
    pub qemu_path: PathBuf,
    #[arg(short, long)]
    pub unix_sock_path: PathBuf,
    #[arg(short, long)]
    pub warden_workdir_path: PathBuf,
    #[arg(short = 't', long, default_value_t = 60)]
    pub realm_connection_wait_time_secs: u64,
}
