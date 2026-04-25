use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputEvent {
    PointerMoveAbsolute {
        x: i32,
        y: i32,
    },
    PointerMoveRelative {
        dx: i32,
        dy: i32,
    },
    PointerButton {
        button: u8,
        pressed: bool,
    },
    Key {
        keycode: u32,
        pressed: bool,
    },
    Text {
        utf8: String,
    },
    GamepadButton {
        gamepad_id: u8,
        button: u16,
        pressed: bool,
    },
    GamepadAxis {
        gamepad_id: u8,
        axis: u16,
        value: f32,
    },
}
