use std::collections::HashMap;

use aya::{
    Ebpf, EbpfLoader, include_bytes_aligned,
    maps::{HashMap as AyaHashMap, RingBuf},
    programs::{SchedClassifier, TcAttachType, tc},
};
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc::UnboundedSender;

use crate::ebpf::triggers;

pub fn load_ebpf(eth_name: &str, trigger_tx: UnboundedSender<String>) {
    crate::ebpf::log::init();
    raise_memlock_rlimit();

    let port_to_services = triggers::load();
    println!(
        "[load_ebpf] eth={eth_name} triggers={} ports={:?}",
        port_to_services.len(),
        port_to_services.keys().collect::<Vec<_>>()
    );

    // Ingress: attach a passive program kept alive for future use.
    {
        let eth_name = eth_name.to_string();
        tokio::spawn(async move {
            match attach(&eth_name, TcAttachType::Ingress) {
                Ok(_bpf) => {
                    println!("[Ingress] attached and idle");
                    std::future::pending::<()>().await
                }
                Err(e) => eprintln!("[Ingress] {e}"),
            }
        });
    }

    // Egress: attach, populate watch ports, async-poll the EVENTS ring buffer.
    {
        let eth_name = eth_name.to_string();
        tokio::spawn(async move {
            let mut bpf = match attach(&eth_name, TcAttachType::Egress) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[Egress] {e}");
                    return;
                }
            };
            if let Err(e) = run_observer(&mut bpf, port_to_services, trigger_tx).await {
                eprintln!("[Egress] {e}");
            }
        });
    }
}

fn raise_memlock_rlimit() {
    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        eprintln!("[load_ebpf] setrlimit(RLIMIT_MEMLOCK) failed: {err} (eBPF load may fail)");
    }
}

fn attach(eth_name: &str, direction: TcAttachType) -> Result<Ebpf, String> {
    let mut loader = EbpfLoader::new();
    if direction == TcAttachType::Egress {
        loader.set_global("IS_EGRESS", &1u8, true);
    }

    let mut bpf = loader
        .load(include_bytes_aligned!(env!("NULLNET_BIN_PATH")))
        .map_err(|e| format!("[{direction:?}] load eBPF bytecode: {e}"))?;
    println!("[{direction:?}] eBPF bytecode loaded");

    match tc::qdisc_add_clsact(eth_name) {
        Ok(()) => println!("[{direction:?}] clsact qdisc added on {eth_name}"),
        Err(e) => println!(
            "[{direction:?}] clsact qdisc add returned: {e} (ok if already present)"
        ),
    }

    let program: &mut SchedClassifier = bpf
        .program_mut("nullnet_filter_ports")
        .ok_or_else(|| {
            format!("[{direction:?}] program 'nullnet_filter_ports' not found in bytecode")
        })?
        .try_into()
        .map_err(|e| format!("[{direction:?}] program is not a SchedClassifier: {e}"))?;

    program
        .load()
        .map_err(|e| format!("[{direction:?}] load program into kernel: {e}"))?;
    println!("[{direction:?}] program loaded into kernel");

    program
        .attach(eth_name, direction)
        .map_err(|e| format!("[{direction:?}] attach to '{eth_name}': {e}"))?;
    println!("[{direction:?}] program attached to {eth_name}");

    Ok(bpf)
}

async fn run_observer(
    bpf: &mut Ebpf,
    port_to_services: HashMap<u16, Vec<String>>,
    trigger_tx: UnboundedSender<String>,
) -> Result<(), String> {
    {
        let mut watch_ports: AyaHashMap<_, u16, u8> = bpf
            .map_mut("WATCH_PORTS")
            .ok_or_else(|| "map 'WATCH_PORTS' not found".to_string())?
            .try_into()
            .map_err(|e| format!("WATCH_PORTS is not a HashMap: {e}"))?;
        for &port in port_to_services.keys() {
            match watch_ports.insert(port, 0u8, 0) {
                Ok(()) => println!("[observer] watching port {port}"),
                Err(e) => eprintln!("[observer] failed to insert watch port {port}: {e}"),
            }
        }
    }

    let events: RingBuf<_> = bpf
        .take_map("EVENTS")
        .ok_or_else(|| "map 'EVENTS' not found".to_string())?
        .try_into()
        .map_err(|e| format!("EVENTS is not a RingBuf: {e}"))?;
    println!("[observer] waiting on EVENTS ring buffer");

    let mut async_fd = AsyncFd::with_interest(events, Interest::READABLE)
        .map_err(|e| format!("registering EVENTS fd with tokio: {e}"))?;

    loop {
        let mut guard = async_fd
            .readable_mut()
            .await
            .map_err(|e| format!("waiting on EVENTS readable: {e}"))?;
        let events = guard.get_inner_mut();
        while let Some(item) = events.next() {
            let bytes: &[u8] = &item;
            if bytes.len() < 2 {
                continue;
            }
            let port = u16::from_le_bytes([bytes[0], bytes[1]]);
            if let Some(services) = port_to_services.get(&port) {
                for service_name in services {
                    if let Err(e) = trigger_tx.send(service_name.clone()) {
                        eprintln!(
                            "[observer] failed to enqueue trigger for '{service_name}': {e}"
                        );
                    } else {
                        println!("[observer] enqueued trigger for '{service_name}'");
                    }
                }
            } else {
                println!("[observer] no services mapped to port {port}");
            }
        }
        guard.clear_ready();
    }
}
