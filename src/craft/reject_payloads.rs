use etherparse::icmpv4::DestUnreachableHeader;
use etherparse::{
    Icmpv4Header, Icmpv4Type, IpNumber, LaxPacketHeaders, LinkExtHeader, LinkHeader, NetHeaders,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use crate::craft::checksums::{ipv4_checksum, tcp_checksum};

/// Sends a proper message to gracefully acknowledge a peer that a packet was rejected,
/// based on the observed protocol:
/// - in case of TCP, a packet with RST and ACK flag is sent
/// - in case of UDP, an ICMP port unreachable message is sent
/// - in case of other protocols, an ICMP host unreachable message is sent
pub async fn send_termination_message(
    packet: &[u8],
    tun_ip: &Ipv4Addr,
    socket: &Arc<UdpSocket>,
    remote_socket: SocketAddr,
) {
    let Ok(headers) = LaxPacketHeaders::from_ethernet(packet) else {
        return;
    };
    let Some(NetHeaders::Ipv4(ip_header, _)) = &headers.net else {
        return;
    };
    let IpNumber(proto) = ip_header.protocol;

    match proto {
        6 => send_tcp_rst(packet, headers, tun_ip, socket, remote_socket).await,
        17 => {
            // port unreachable
            let icmp_type = Icmpv4Type::DestinationUnreachable(DestUnreachableHeader::Port);
            send_destination_unreachable(packet, headers, tun_ip, socket, icmp_type, remote_socket)
                .await;
        }
        _ => {
            // host unreachable
            let icmp_type = Icmpv4Type::DestinationUnreachable(DestUnreachableHeader::Host);
            send_destination_unreachable(packet, headers, tun_ip, socket, icmp_type, remote_socket)
                .await;
        }
    }
}

async fn send_destination_unreachable(
    packet: &[u8],
    headers: LaxPacketHeaders<'_>,
    tun_ip: &Ipv4Addr,
    socket: &Arc<UdpSocket>,
    icmp_type: Icmpv4Type,
    remote_socket: SocketAddr,
) {
    let Some(LinkHeader::Ethernet2(mut ethernet_header)) = headers.link else {
        return;
    };
    let src_mac_orig = ethernet_header.source;
    ethernet_header.source = ethernet_header.destination;
    ethernet_header.destination = src_mac_orig;
    let ethernet_header_bytes = ethernet_header.to_bytes();

    let link_exts = &headers.link_exts;
    let link_exts_bytes: Vec<u8> = link_exts
        .iter()
        .flat_map(|ext| match ext {
            LinkExtHeader::Vlan(e) => e.to_bytes().to_vec(),
            LinkExtHeader::Macsec(e) => e.to_bytes().to_vec(),
        })
        .collect();

    let Some(NetHeaders::Ipv4(mut ip_header, _)) = headers.net else {
        return;
    };
    ip_header.destination = ip_header.source;
    ip_header.source = tun_ip.octets();
    ip_header.total_len = 28; // TODO??? 56 = 20 (ip header) + 8 (icmp header) + 28 (original ip header + first 8 bytes of data)
    ip_header.header_checksum = ip_header.calc_header_checksum();
    let ip_header_bytes = ip_header.to_bytes();

    let mut icmp_header = Icmpv4Header::new(icmp_type);
    let _ = icmp_header.update_checksum(&[]); // empty payload for now
    let icmp_header_bytes = icmp_header.to_bytes();

    #[rustfmt::skip]
    let pkt_response = [
        &ethernet_header_bytes[..],
        &link_exts_bytes[..],
        &ip_header_bytes[..],
        &icmp_header_bytes[..],
    ].concat();

    socket
        .send_to(&pkt_response, remote_socket)
        .await
        .unwrap_or(0);
}

async fn send_tcp_rst(
    packet: &[u8],
    headers: LaxPacketHeaders<'_>,
    tun_ip: &Ipv4Addr,
    socket: &Arc<UdpSocket>,
    remote_socket: SocketAddr,
) {
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
    pkt_response[12..16].clone_from_slice(&tun_ip.octets()); // my IP

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
            u32::try_from(packet.len()).unwrap_or_default() - 20 - (u32::from(packet[32]) >> 4) * 4;
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
        .send_to(&pkt_response, remote_socket)
        .await
        .unwrap_or(0);
}
