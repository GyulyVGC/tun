use std::net::Ipv4Addr;
use std::process::Command;

const CHAIN: &str = "NULLNET_DNAT";
const HOOK_CHAINS: [&str; 2] = ["OUTPUT", "PREROUTING"];
const PROTOS: [&str; 2] = ["tcp", "udp"];

/// Resets the private DNAT chain and conntrack so a fresh process start
/// inherits no stale state from a previous run. Idempotent.
pub(crate) fn init() {
    // create our chain (no-op if it already exists)
    let _ = sudo(&["iptables", "-t", "nat", "-N", CHAIN]);
    // flush any rules left over from a previous run
    let _ = sudo(&["iptables", "-t", "nat", "-F", CHAIN]);
    // hook the chain from OUTPUT and PREROUTING (idempotent via -C check)
    for chain in HOOK_CHAINS {
        let already = sudo(&["iptables", "-t", "nat", "-C", chain, "-j", CHAIN])
            .map(|s| s.success())
            .unwrap_or(false);
        if !already {
            let _ = sudo(&["iptables", "-t", "nat", "-A", chain, "-j", CHAIN]);
        }
    }
    // drop any conntrack flows that may have been NAT'd through stale rules
    let _ = sudo(&["conntrack", "-F"]);
    println!("[dnat] init: chain {CHAIN} ready, conntrack flushed");
}

pub(crate) fn install(port: u16, overlay_ip: Ipv4Addr) {
    for proto in PROTOS {
        run_iptables("-A", proto, port, overlay_ip);
    }
    flush_conntrack(port);
}

pub(crate) fn remove(port: u16, overlay_ip: Ipv4Addr) {
    for proto in PROTOS {
        run_iptables("-D", proto, port, overlay_ip);
    }
    flush_conntrack(port);
}

fn run_iptables(action: &str, proto: &str, port: u16, overlay_ip: Ipv4Addr) {
    let port_s = port.to_string();
    let target = format!("{overlay_ip}:{port}");
    let status = sudo(&[
        "iptables",
        "-t",
        "nat",
        action,
        CHAIN,
        "-p",
        proto,
        "--dport",
        &port_s,
        "-j",
        "DNAT",
        "--to-destination",
        &target,
    ]);
    match status {
        Ok(s) if s.success() => {
            println!("[dnat] iptables {action} {CHAIN} {proto}/{port} -> {target}");
        }
        Ok(s) => {
            eprintln!("[dnat] iptables {action} {CHAIN} {proto}/{port} -> {target} exited {s}");
        }
        Err(e) => {
            eprintln!("[dnat] iptables {action} {CHAIN} {proto}/{port} -> {target}: {e}");
        }
    }
}

fn flush_conntrack(port: u16) {
    let port_s = port.to_string();
    for proto in PROTOS {
        let _ = sudo(&["conntrack", "-D", "-p", proto, "--dport", &port_s]);
    }
}

fn sudo(args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    Command::new("sudo").args(args).status()
}
