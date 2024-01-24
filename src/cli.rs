use clap::Parser;
use std::net::IpAddr;

/// TUN-based networking in Rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// IP address used as source of the socket
    #[arg(long)]
    pub source: Option<IpAddr>,
    /// Whether to produce logs (console and SQLite)
    #[arg(long, default_value_t = false)]
    pub log: bool,
    /// Maximum Transmission Unit (bytes)
    #[arg(long, default_value_t = 42500)]
    pub mtu: usize,
    /// Path of the file defining firewall rules
    #[arg(long, default_value_t = String::from("firewall/firewall.txt"))]
    pub firewall_path: String,
    /// Number of asynchronous tasks to use (AKA coroutines)
    #[arg(long, default_value_t = 2)]
    pub num_tasks: usize,
}
