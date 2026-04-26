use std::{
    io::{Read, Write},
    net::TcpStream,
    os::unix::net::UnixStream,
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sunbeam_common::input::InputEvent;

#[derive(Debug, Parser)]
#[command(name = "sunbeamctl")]
#[command(about = "CLI control tool for Sunbeam host")]
struct Cli {
    /// Host control socket path
    #[arg(long, default_value = "/tmp/sunbeam.sock.ctl")]
    control_socket: PathBuf,

    /// Optional host:port for remote TCP control mode.
    #[arg(long)]
    tcp: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Sessions,
    Select { agent_id: String },
    MoveMouse { x: i32, y: i32 },
    MouseButton { button: u8, action: ButtonAction },
    Key { keycode: u32, action: ButtonAction },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ButtonAction {
    Press,
    Release,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let request = match cli.command {
        Command::Sessions => "sessions".to_string(),
        Command::Select { agent_id } => format!("select {agent_id}"),
        Command::MoveMouse { x, y } => {
            let event = InputEvent::PointerMoveAbsolute { x, y };
            format!("input {}", serde_json::to_string(&event)?)
        }
        Command::MouseButton { button, action } => {
            let event = InputEvent::PointerButton {
                button,
                pressed: matches!(action, ButtonAction::Press),
            };
            format!("input {}", serde_json::to_string(&event)?)
        }
        Command::Key { keycode, action } => {
            let event = InputEvent::Key {
                keycode,
                pressed: matches!(action, ButtonAction::Press),
            };
            format!("input {}", serde_json::to_string(&event)?)
        }
    };

    let response = if let Some(addr) = &cli.tcp {
        send_control_command_tcp(addr, &request)?
    } else {
        send_control_command_unix(&cli.control_socket, &request)?
    };
    println!("{response}");
    Ok(())
}

fn send_control_command_unix(control_socket: &PathBuf, request: &str) -> Result<String> {
    let mut stream = UnixStream::connect(control_socket)
        .with_context(|| format!("connecting to control socket {}", control_socket.display()))?;
    stream
        .write_all(request.as_bytes())
        .context("writing request to control socket")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .context("signaling end-of-request to host")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("reading control response")?;

    Ok(response)
}

fn send_control_command_tcp(addr: &str, request: &str) -> Result<String> {
    let mut stream =
        TcpStream::connect(addr).with_context(|| format!("connecting to control tcp {addr}"))?;
    stream
        .write_all(request.as_bytes())
        .context("writing request to control tcp")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .context("signaling end-of-request to host")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("reading control response")?;

    Ok(response)
}
