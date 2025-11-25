use etherparse::icmpv4::DestUnreachableHeader;
use etherparse::{
    Icmpv4Header, Icmpv4Type, IpNumber, LaxPacketHeaders, LinkExtHeader, LinkHeader, NetHeaders,
    TransportHeader,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use crate::craft::checksums::tcp_checksum;

/// Sends a proper message to gracefully acknowledge a peer that a packet was rejected,
/// based on the observed protocol:
/// - in case of TCP, a packet with RST and ACK flag is sent
/// - in case of UDP, an ICMP port unreachable message is sent
/// - in case of other protocols, an ICMP host unreachable message is sent
pub async fn send_termination_message(
    packet: &[u8],
    tun_mac: &[u8; 6],
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
        6 => send_tcp_rst(packet, headers, tun_mac, tun_ip, socket, remote_socket).await,
        17 => {
            // port unreachable
            let icmp_type = Icmpv4Type::DestinationUnreachable(DestUnreachableHeader::Port);
            send_destination_unreachable(
                packet,
                headers,
                tun_mac,
                tun_ip,
                socket,
                icmp_type,
                remote_socket,
            )
            .await;
        }
        _ => {
            // host unreachable
            let icmp_type = Icmpv4Type::DestinationUnreachable(DestUnreachableHeader::Host);
            send_destination_unreachable(
                packet,
                headers,
                tun_mac,
                tun_ip,
                socket,
                icmp_type,
                remote_socket,
            )
            .await;
        }
    }
}

async fn send_destination_unreachable(
    packet: &[u8],
    headers: LaxPacketHeaders<'_>,
    tun_mac: &[u8; 6],
    tun_ip: &Ipv4Addr,
    socket: &Arc<UdpSocket>,
    icmp_type: Icmpv4Type,
    remote_socket: SocketAddr,
) {
    let Some(LinkHeader::Ethernet2(mut ethernet_header)) = headers.link else {
        return;
    };
    ethernet_header.destination = ethernet_header.source;
    ethernet_header.source = *tun_mac;
    ethernet_header.source = ethernet_header.destination;
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
    tun_mac: &[u8; 6],
    tun_ip: &Ipv4Addr,
    socket: &Arc<UdpSocket>,
    remote_socket: SocketAddr,
) {
    let Some(LinkHeader::Ethernet2(mut ethernet_header)) = headers.link else {
        return;
    };
    ethernet_header.destination = ethernet_header.source;
    ethernet_header.source = *tun_mac;
    ethernet_header.source = ethernet_header.destination;
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
    ip_header.total_len = 40;
    ip_header.header_checksum = ip_header.calc_header_checksum();
    let ip_header_bytes = ip_header.to_bytes();

    let Some(TransportHeader::Tcp(mut tcp_header)) = headers.transport else {
        return;
    };
    let src_port_orig = tcp_header.source_port;
    tcp_header.source_port = tcp_header.destination_port;
    tcp_header.destination_port = src_port_orig;
    tcp_header.sequence_number = tcp_header.acknowledgment_number;
    // tcp_header.acknowledgment_number TODO
    tcp_header.ack = true;
    tcp_header.rst = true;
    tcp_header.cwr = false;
    tcp_header.ns = false;
    tcp_header.psh = false;
    tcp_header.syn = false;
    tcp_header.fin = false;
    tcp_header.urg = false;
    tcp_header.urgent_pointer = 0;
    tcp_header.checksum = tcp_header
        .calc_checksum_ipv4(&ip_header, &[])
        .unwrap_or_default();
    let tcp_header_bytes = tcp_header.to_bytes();

    #[rustfmt::skip]
    let mut pkt_response = [

        // TCP header
        0x00, 0x00,                         // src port (will be set later)
        0x00, 0x00,                         // dst port (will be set later)
        0x00, 0x00, 0x00, 0x00,             // sequence number (will be set later)
        0x00, 0x00, 0x00, 0x00,     // ACK number (will be set later)
        0x50,                               // data offset & reserved bits
        0b_0001_0100,                       // flags: ACK, RST
        0x00, 0x00,                         // window size (will be set later)
        0x00, 0x00,                         // checksum (will be set later)
        0x00, 0x00,                         // urgent pointer
    ];

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

    socket
        .send_to(&pkt_response, remote_socket)
        .await
        .unwrap_or(0);
}
