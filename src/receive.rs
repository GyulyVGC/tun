use crate::peers::TUN_TO_SOCKET;
use crate::socket_frame::SocketFrame;
use nullnet_firewall::{Firewall, FirewallAction, FirewallDirection};
use std::io::Write;
use std::net::UdpSocket;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use tun::platform::posix::Writer;

pub fn receive(
    mut device: Writer,
    socket: &Arc<UdpSocket>,
    firewall: &Arc<RwLock<Firewall>>,
    tun_ip: &IpAddr,
) {
    let mut socket_frame = SocketFrame::new();
    loop {
        // wait until there is an incoming packet on the socket (packets on the socket are raw IP)
        (socket_frame.actual_bytes, _) = socket
            .recv_from(&mut socket_frame.frame)
            .unwrap_or_else(|_| (0, SocketAddr::from_str("0.0.0.0:0").unwrap()));

        if socket_frame.actual_bytes > 0 {
            match firewall
                .read()
                .unwrap()
                .resolve_packet(socket_frame.actual_frame(), FirewallDirection::IN)
            {
                FirewallAction::ACCEPT => {
                    // write packet to the kernel
                    let os_buf = socket_frame.to_os_buf();
                    #[allow(clippy::needless_borrow)]
                    device.write_all(&os_buf).unwrap_or(());
                }
                FirewallAction::REJECT => {
                    match socket_frame.actual_frame()[9] {
                        6 => send_tcp_rst(socket_frame.actual_frame(), tun_ip, socket),
                        // 17 => udp...
                        _ => send_destination_unreachable(
                            socket_frame.actual_frame(),
                            tun_ip,
                            socket,
                        ),
                    };
                }
                FirewallAction::DENY => {}
            }
        }
    }
}

fn send_destination_unreachable(packet: &[u8], tun_ip: &IpAddr, socket: &Arc<UdpSocket>) {
    if packet.len() < 28 {
        return;
    }

    let rejected_sender_ip = &packet[12..16];
    let Some(dest_socket) = TUN_TO_SOCKET.get(rejected_sender_ip) else {
        return;
    };

    let mut pkt_response = [
        // ipv4 header
        0x45, 0x00, // version, header length, congestion
        0x00, 0x00, // length (will be set later)
        0x00, 0x00, 0x00, 0x00, // identification, fragmentation
        0x40, 0x01, // ttl and protocol
        0x00, 0x00, // header checksum (will be set later)
        0x00, 0x00, 0x00, 0x00, // source (will be set later)
        0x00, 0x00, 0x00, 0x00, // dest (will be set later)
        // icmp header
        0x03, 0x01, // destination host unreachable
        0x00, 0x00, // checksum (will be set later)
        0x00, 0x00, 0x00,
        0x00, // unused
              // the original ip header and the first 64 bits
              // of the original datagram will be included here
    ];

    // length
    pkt_response[2] = 0x00; // 56 bytes (0x0038)
    pkt_response[3] = 0x38;

    // source
    let IpAddr::V4(tun_ip_v4) = tun_ip else {
        panic!("IPv6 not supported yet as TUN address...");
    };
    pkt_response[12..16].clone_from_slice(&tun_ip_v4.octets()); // my IP

    // dest
    pkt_response[16..20].clone_from_slice(&packet[12..16]); // sender of the rejected packet

    // ip header checksum
    let ip_checksum = calc_ipv4_checksum(&pkt_response[..20]);
    pkt_response[10] = (ip_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[11] = (ip_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    // rest of the packet: original IP header and first 8 bytes of data
    let pkt_response_final = &mut [&pkt_response[..], &packet[..28]].concat()[..];

    // icmp checksum
    let icmp_checksum = calc_icmp_checksum(&pkt_response_final[20..]);
    pkt_response_final[22] = (icmp_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response_final[23] = (icmp_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    socket.send_to(pkt_response_final, dest_socket).unwrap_or(0);
}

fn send_tcp_rst(packet: &[u8], tun_ip: &IpAddr, socket: &Arc<UdpSocket>) {
    if packet.len() < 40 {
        return;
    }

    let rejected_sender_ip = &packet[12..16];
    let Some(dest_socket) = TUN_TO_SOCKET.get(rejected_sender_ip) else {
        return;
    };

    #[rustfmt::skip]
    let mut pkt_response = [
        // ipv4 header
        0x45, 0x00, // version, header length, congestion
        0x00, 0x00, // length (will be set later)
        0x00, 0x00, 0x00, 0x00, // identification, fragmentation
        0x40, 0x06, // ttl and protocol
        0x00, 0x00, // header checksum (will be set later)
        0x00, 0x00, 0x00, 0x00, // source (will be set later)
        0x00, 0x00, 0x00, 0x00, // dest (will be set later)
        // TCP header
        0x00, 0x00, // src port (will be set later)
        0x00, 0x00, // dst port (will be set later)
        0x00, 0x00, 0x00, 0x00, // sequence number: received seq. num. + payload len (will be set later)
        0x00, 0x00, 0x00, 0x00, // ACK number
        0x50,       // data offset & reserved bits
        0b00000100, // flags: RST
        0x00, 0x00, // window size (will be set later)
        0x00, 0x00, // checksum (will be set later)
        0x00, 0x00, // urgent pointer
    ];

    // length
    pkt_response[2] = 0x00; // 40 bytes (0x0028)
    pkt_response[3] = 0x28;

    // source
    let IpAddr::V4(tun_ip_v4) = tun_ip else {
        panic!("IPv6 not supported yet as TUN address...");
    };
    pkt_response[12..16].clone_from_slice(&tun_ip_v4.octets()); // my IP

    // dest
    pkt_response[16..20].clone_from_slice(&packet[12..16]); // sender of the rejected packet

    // ip header checksum
    let ip_checksum = calc_ipv4_checksum(&pkt_response[..20]);
    pkt_response[10] = (ip_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[11] = (ip_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    // src port
    pkt_response[20..22].clone_from_slice(&packet[22..24]); // dst port of the rejected packet

    // dst port
    pkt_response[22..24].clone_from_slice(&packet[20..22]); // src port of the rejected packet

    // sequence number
    let mut seq = ((packet[24] as u32) << 24)
        + ((packet[25] as u32) << 16)
        + ((packet[26] as u32) << 8)
        + ((packet[27] as u32) << 0);
    let rejected_payload_len = packet.len() as u32 - 20 - (packet[32] as u32 >> 4) * 4;
    seq = seq.wrapping_add(rejected_payload_len);
    pkt_response[24..28].clone_from_slice(&seq.to_be_bytes());

    // window size
    pkt_response[34..36].clone_from_slice(&packet[34..36]);

    // TCP header checksum
    let tcp_checksum = calc_tcp_checksum(&pkt_response[..40]);
    pkt_response[36] = (tcp_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[37] = (tcp_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    socket.send_to(&pkt_response, dest_socket).unwrap_or(0);
}

fn calc_ipv4_checksum(ipv4_header: &[u8]) -> u16 {
    assert_eq!(ipv4_header.len() % 2, 0);
    let mut checksum = 0;
    for i in 0..ipv4_header.len() / 2 {
        if i == 5 {
            // Assume checksum field is set to 0
            continue;
        }
        checksum += (u32::from(ipv4_header[i * 2]) << 8) + u32::from(ipv4_header[i * 2 + 1]);
        if checksum > 0xffff {
            checksum = (checksum & 0xffff) + 1;
        }
    }
    !u16::try_from(checksum).unwrap()
}

fn calc_icmp_checksum(icmp_data: &[u8]) -> u16 {
    assert_eq!(icmp_data.len() % 2, 0);
    let mut checksum = 0;
    for i in 0..icmp_data.len() / 2 {
        if i == 1 {
            // Assume checksum field is set to 0
            continue;
        }
        checksum += (u32::from(icmp_data[i * 2]) << 8) + u32::from(icmp_data[i * 2 + 1]);
        if checksum > 0xffff {
            checksum = (checksum & 0xffff) + 1;
        }
    }
    !u16::try_from(checksum).unwrap()
}

fn calc_tcp_checksum(ip_tcp_headers: &[u8]) -> u16 {
    assert_eq!(ip_tcp_headers.len() % 2, 0);
    let mut checksum = 0;
    checksum += 26; // protocol and TCP segment len (6 + 20)
    for i in 6..ip_tcp_headers.len() / 2 {
        if i == 18 {
            // Assume checksum field is set to 0
            continue;
        }
        checksum += (u32::from(ip_tcp_headers[i * 2]) << 8) + u32::from(ip_tcp_headers[i * 2 + 1]);
        if checksum > 0xffff {
            checksum = (checksum & 0xffff) + 1;
        }
    }
    !u16::try_from(checksum).unwrap()
}
