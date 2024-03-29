use clap::Parser;

/// TUN-based networking in Rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Maximum Transmission Unit (bytes)
    #[arg(long, default_value_t = 42500)]
    pub mtu: u16,
    /// Path of the file defining firewall rules (it should be inside a dedicated folder)
    #[arg(long, default_value_t = String::from("firewall/firewall.txt"))]
    pub firewall_path: String,
    // /// Path of the SQLite database storing traffic logs TODO
    // #[arg(long, default_value_t = String::from("log.sqlite"))]
    // pub log_path: String,
    // /// Path of the SQLite database storing active peers
    // #[arg(long, default_value_t = String::from("peers.sqlite"))]
    // pub peers_path: String,
    /// Number of asynchronous tasks to use (AKA coroutines)
    #[arg(long, default_value_t = 2, value_parser=clap::value_parser!(u8).range(2..))]
    pub num_tasks: u8,
}
