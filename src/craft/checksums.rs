/// Computes the IPv4 header checksum.
pub(super) fn ipv4_checksum(ipv4_header: &[u8]) -> u16 {
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
    !u16::try_from(checksum).unwrap_or_default()
}

/// Computes the ICMP header checksum.
pub(super) fn icmp_checksum(icmp_data: &[u8]) -> u16 {
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
    !u16::try_from(checksum).unwrap_or_default()
}

/// Computes the TCP checksum.
pub(super) fn tcp_checksum(ip_tcp_headers: &[u8]) -> u16 {
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
    !u16::try_from(checksum).unwrap_or_default()
}
