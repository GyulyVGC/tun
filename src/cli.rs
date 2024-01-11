use clap::Parser;
use std::net::IpAddr;

/// TUN-based networking in Rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// IP address used as source of the socket
    #[arg(long)]
    pub source: IpAddr,
    /// Whether to produce logs (console and SQLite) or not
    #[arg(long, default_value_t = false)]
    pub log: bool,
    /// Maximum Transmission Unit (bytes)
    #[arg(long, default_value_t = 1500 - 20 - 8)]
    pub mtu: usize,
    /// Path of the file defining firewall rules
    #[arg(long, default_value_t = String::from("./firewall.txt"))]
    pub firewall_path: String,
}
