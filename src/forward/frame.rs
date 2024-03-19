/// Representation of a network packet transiting on sockets.
/// All packets of this kind are raw IP.
pub struct Frame {
    pub frame: [u8; 65536],
    pub size: usize,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            frame: [0; 65536],
            size: 0,
        }
    }

    pub fn pkt_data(&self) -> &[u8] {
        &self.frame[..self.size]
    }
}
