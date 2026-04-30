# Pure eBPF vs. eBPF + Netfilter NAT (our design)

## Per-concern comparison

| Concern | Pure eBPF | Our hybrid |
|---|---|---|
| **Flow state** | Custom `LRU_HASH` keyed on 5-tuple; you write it | conntrack: kernel-resident, automatic |
| **First-packet rule eval** | Custom matcher in BPF | iptables rule, evaluated once per flow |
| **Reverse-direction NAT** | Second TC ingress program with reverse lookup + rewrite | conntrack derives reverse from forward automatically |
| **L3/L4 checksum recomputation** | `bpf_l3/l4_csum_replace` with `BPF_F_PSEUDO_HDR` / `BPF_F_MARK_MANGLED_0` flags | `DNAT` target handles it, including HW offload |
| **TCP state tracking** | Parse SYN/FIN/RST; run state machine per flow | conntrack tracks states + per-state timeouts |
| **Connection eviction / GC** | LRU map or userspace sweeper | conntrack GC, state-aware timeouts |
| **IP fragmentation** | No reassembly helper; stitch yourself or blackhole | conntrack reassembles |
| **ICMP-related (Dest Unreachable, etc.)** | Parse inner packet; rewrite inner + outer + ICMP checksum | conntrack translates automatically |
| **ALGs (FTP, SIP, PPTP)** | Per-protocol payload parsers in BPF | conntrack helpers exist |
| **Per-CPU concurrency** | Single-elem atomics OK; multi-step needs CAS | Conntrack handles it |
| **Routing after rewrite** | `bpf_redirect_neigh` + ifindex/MAC plumbing | Kernel re-routes; picks bridge → VXLAN |
| **Observability** | Custom map dumpers, `bpftool` | `iptables -L`, `conntrack -L`, `tcpdump`, audit logs |
| **Kernel-version compatibility** | Helper gating (`bpf_redirect_neigh` 5.10+, `bpf_loop` 5.17+, ringbuf 5.8+) | Stable Netfilter API for 20+ years |
| **Build complexity** | Separate `bpfel-unknown-none` target, separate toolchain, xtask glue | Plain Rust + shell-out |
| **Code size** | ~thousands of lines (BPF + userspace control plane) | ~80 lines of Rust |
| **Failure mode** | Verifier surprises, silent blackholes from bad checksums | Battle-tested target |
| **Atomic rule updates** | Per-element map ops only | nftables sets / iptables rule replace |

## What we keep from eBPF

| Capability | Where it lives |
|---|---|
| Cheap kernel-side packet observation | TC classifier on egress (`nullnet_filter_ports`) |
| Watched-port matching | `WATCH_PORTS` BPF map |
| Userspace events without per-packet syscalls | `EVENTS` ring buffer |
| Strategic eBPF surface for future use | Idle ingress program kept attached |

## Net advantages of the hybrid

| Advantage | Why |
|---|---|
| **In-kernel data path** | Both eBPF and Netfilter run kernel-side; no userspace on the packet path |
| **No conntrack reimplementation** | 20+ years of kernel work — state, reverse NAT, fragments, ICMP, ALGs — reused |
| **~80 lines of Rust for the redirect** | One file (`commands/dnat.rs`) + a small lifecycle struct |
| **Standard Linux tooling** | `iptables -L`, `conntrack -L`, `tcpdump` work out of the box |
| **Hardware checksum offload preserved** | Kernel's `DNAT` cooperates with NIC offload |
| **Reboot-safe state** | Private `NULLNET_DNAT` chain; `dnat::init()` flushes on every start |
| **Idempotent triggers** | `TriggersState` ensures one `backend_trigger` per service per VXLAN lifetime |
| **Clear separation of roles** | eBPF where it wins (observation); kernel NAT where it would otherwise be a worse rewrite |
