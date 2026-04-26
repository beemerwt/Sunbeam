use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};

use crate::{frame::FrameDescriptor, input::InputEvent, session::SessionInfo};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WireMessage {
    Register {
        session: SessionInfo,
    },
    Frame {
        descriptor: FrameDescriptor,
        payload_len: u32,
    },
    Input {
        event: InputEvent,
    },
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WirePacket {
    pub message: WireMessage,
}

pub fn write_packet<W: Write>(
    writer: &mut W,
    packet: &WirePacket,
    payload: Option<&[u8]>,
) -> io::Result<()> {
    let header = serde_json::to_vec(packet).map_err(io::Error::other)?;
    let header_len = (header.len() as u32).to_le_bytes();
    writer.write_all(&header_len)?;
    writer.write_all(&header)?;

    if let Some(bytes) = payload {
        writer.write_all(bytes)?;
    }

    writer.flush()?;
    Ok(())
}

pub fn read_packet<R: Read>(reader: &mut R) -> io::Result<(WirePacket, Vec<u8>)> {
    let mut header_len = [0u8; 4];
    reader.read_exact(&mut header_len)?;
    let header_len = u32::from_le_bytes(header_len) as usize;

    let mut header = vec![0u8; header_len];
    reader.read_exact(&mut header)?;
    let packet: WirePacket = serde_json::from_slice(&header).map_err(io::Error::other)?;

    let payload_len = match &packet.message {
        WireMessage::Frame { payload_len, .. } => *payload_len as usize,
        _ => 0,
    };

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload)?;
    }

    Ok((packet, payload))
}
