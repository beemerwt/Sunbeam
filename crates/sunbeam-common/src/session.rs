use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionCapabilities {
    pub capture_root: bool,
    pub capture_window: bool,
    pub inject_keyboard_mouse: bool,
    pub inject_gamepad: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionInfo {
    pub agent_id: String,
    pub backend: String,
    pub session_name: String,
    pub display: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub capabilities: SessionCapabilities,
}
