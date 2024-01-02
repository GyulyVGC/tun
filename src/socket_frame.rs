#[cfg(target_os = "macos")]
use crate::os_frame::OsFrame;

/// Representation of a network packet transiting on sockets
/// All packets of this kind must be in raw IP form
pub struct SocketFrame {
    pub frame: [u8; SocketFrame::MAX_SIZE],
    pub actual_bytes: usize,
}

impl SocketFrame {
    const MAX_SIZE: usize = 4096;

    pub fn new() -> Self {
        Self {
            frame: [0; Self::MAX_SIZE],
            actual_bytes: 0,
        }
    }

    pub fn actual_frame(&self) -> &[u8] {
        &self.frame[..self.actual_bytes]
    }

    #[cfg(not(target_os = "macos"))]
    pub fn to_os_buf(&self) -> &[u8] {
        let os_buf = self.actual_frame();
        os_buf
    }

    #[cfg(target_os = "macos")]
    pub fn to_os_buf(&self) -> Box<[u8]> {
        let os_buf = &[OsFrame::AF_INET_HEADER, self.actual_frame()].concat()[..];
        os_buf.into()
    }
}