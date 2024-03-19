/// Representation of a network packet that can be interpreted by the target specific Operating System.
/// Packets of this kind are raw IP in Windows and Linux, and null/loopback in macOS.
pub struct OsFrame {
    pub frame: [u8; 65536],
    pub actual_bytes: usize,
}

impl OsFrame {
    // #[cfg(target_os = "macos")]
    // pub const AF_INET_HEADER: &'static [u8] = &[0, 0, 0, 2];

    pub fn new() -> Self {
        Self {
            frame: [0; 65536],
            actual_bytes: 0,
        }
    }

    fn actual_frame_from_byte(&self, byte: usize) -> &[u8] {
        &self.frame[byte..self.actual_bytes]
    }

    pub fn to_socket_buf(&self) -> &[u8] {
        // #[cfg(not(target_os = "macos"))]
        let socket_buf = self.actual_frame_from_byte(0);

        // #[cfg(target_os = "macos")]
        // let socket_buf = self.actual_frame_from_byte(4);

        socket_buf
    }
}
