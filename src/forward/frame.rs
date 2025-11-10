/// Representation of a network packet transiting on sockets.
/// All packets of this kind are raw IP.
pub struct Frame {
    pub frame: Box<[u8]>,
    pub size: usize,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            frame: vec![0; u16::MAX as usize].into_boxed_slice(),
            size: 0,
        }
    }

    pub fn pkt_data(&self) -> &[u8] {
        &self.frame[..self.size]
    }
}
