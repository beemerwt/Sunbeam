#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
}

#[derive(Debug, Clone)]
pub struct EncodedVideoPacket {
    pub codec: VideoCodec,
    pub timestamp_micros: u64,
    pub keyframe: bool,
    pub bytes: Vec<u8>,
}
