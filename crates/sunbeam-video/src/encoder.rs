use anyhow::Result;

use crate::types::EncodedVideoPacket;

pub trait EncodedVideoSink {
    fn push_packet(&mut self, packet: EncodedVideoPacket) -> Result<()>;
}
