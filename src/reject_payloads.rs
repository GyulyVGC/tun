use crate::checksums::{icmp_checksum, ipv4_checksum, tcp_checksum};
use crate::peers::TUN_TO_SOCKET;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

pub async fn send_termination_message(packet: &[u8], tun_ip: &IpAddr, socket: &Arc<UdpSocket>) {
    let Some(proto) = packet.get(9) else {
        return;
    };
    match proto {
        6 => send_tcp_rst(packet, tun_ip, socket).await,
        17 => {
            send_destination_unreachable(
                packet, tun_ip, socket, 3, // port unreachable
            )
            .await;
        }
        _ => {
            send_destination_unreachable(
                packet, tun_ip, socket, 1, // host unreachable
            )
            .await;
        }
    };
}

async fn send_destination_unreachable(
    packet: &[u8],
    tun_ip: &IpAddr,
    socket: &Arc<UdpSocket>,
    unreachable_code: u8,
) {
    if packet.len() < 28 {
        return;
    }

    let rejected_sender_ip = &packet[12..16];
    let Some(dest_socket) = TUN_TO_SOCKET.get(rejected_sender_ip) else {
        return;
    };

    #[rustfmt::skip]
    let mut pkt_response = [
        // ipv4 header
        0x45, 0x00,                 // version, header length, congestion
        0x00, 0x00,                 // length (will be set later)
        0x00, 0x00, 0x00, 0x00,     // identification, fragmentation
        0x40, 0x01,                 // ttl and protocol
        0x00, 0x00,                 // header checksum (will be set later)
        0x00, 0x00, 0x00, 0x00,     // source (will be set later)
        0x00, 0x00, 0x00, 0x00,     // dest (will be set later)
        // icmp header
        0x03, unreachable_code,     // destination host unreachable
        0x00, 0x00,                 // checksum (will be set later)
        0x00, 0x00, 0x00, 0x00,     // unused
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
    let ip_checksum = ipv4_checksum(&pkt_response[..20]);
    pkt_response[10] = (ip_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[11] = (ip_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    // rest of the packet: original IP header and first 8 bytes of data
    let pkt_response_final = &mut [&pkt_response[..], &packet[..28]].concat()[..];

    // icmp checksum
    let icmp_checksum = icmp_checksum(&pkt_response_final[20..]);
    pkt_response_final[22] = (icmp_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response_final[23] = (icmp_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    socket
        .send_to(pkt_response_final, dest_socket)
        .await
        .unwrap_or(0);
}

async fn send_tcp_rst(packet: &[u8], tun_ip: &IpAddr, socket: &Arc<UdpSocket>) {
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
        0x45, 0x00,                 // version, header length, congestion
        0x00, 0x00,                 // length (will be set later)
        0x00, 0x00, 0x00, 0x00,     // identification, fragmentation
        0x40, 0x06,                 // ttl and protocol
        0x00, 0x00,                 // header checksum (will be set later)
        0x00, 0x00, 0x00, 0x00,     // source (will be set later)
        0x00, 0x00, 0x00, 0x00,     // dest (will be set later)
        // TCP header
        0x00, 0x00,                 // src port (will be set later)
        0x00, 0x00,                 // dst port (will be set later)
        0x00, 0x00, 0x00, 0x00,     // sequence number (will be set later)
        0x00, 0x00, 0x00, 0x00,     // ACK number (will be set later)
        0x50,                       // data offset & reserved bits
        0b_0001_0100,                 // flags: ACK, RST
        0x00, 0x00,                 // window size (will be set later)
        0x00, 0x00,                 // checksum (will be set later)
        0x00, 0x00,                 // urgent pointer
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
    let ip_checksum = ipv4_checksum(&pkt_response[..20]);
    pkt_response[10] = (ip_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[11] = (ip_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    // src port
    pkt_response[20..22].clone_from_slice(&packet[22..24]); // dst port of the rejected packet

    // dst port
    pkt_response[22..24].clone_from_slice(&packet[20..22]); // src port of the rejected packet

    // sequence number
    pkt_response[24..28].clone_from_slice(&packet[28..32]);

    // ACK number
    let mut ack = (u32::from(packet[24]) << 24)
        + (u32::from(packet[25]) << 16)
        + (u32::from(packet[26]) << 8)
        + u32::from(packet[27]);
    if packet[33] & 0b_0000_0010 == 0b_0000_0010 {
        // SYN was set in the rejected packet
        ack = ack.wrapping_add(1);
    } else {
        // SYN wasn't set in the rejected packet
        let rejected_payload_len =
            u32::try_from(packet.len()).unwrap() - 20 - (u32::from(packet[32]) >> 4) * 4;
        ack = ack.wrapping_add(rejected_payload_len);
    }
    pkt_response[28..32].clone_from_slice(&ack.to_be_bytes());

    // window size
    pkt_response[34..36].clone_from_slice(&packet[34..36]);

    // TCP header checksum
    let tcp_checksum = tcp_checksum(&pkt_response[..40]);
    pkt_response[36] = (tcp_checksum >> 8) as u8; // calculated checksum is little-endian; checksum field is big-endian
    pkt_response[37] = (tcp_checksum & 0xff) as u8; // calculated checksum is little-endian; checksum field is big-endian

    socket
        .send_to(&pkt_response, dest_socket)
        .await
        .unwrap_or(0);
}
