use serde::{Deserialize, Serialize};

use crate::{frame::FrameDescriptor, input::InputEvent, session::SessionInfo};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentMessage {
    Register { session: SessionInfo },
    FrameReady(FrameDescriptor),
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HostMessage {
    SelectSession { agent_id: String },
    Input(InputEvent),
    StartCapture,
    StopCapture,
}
