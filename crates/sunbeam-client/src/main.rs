use std::{
    io::{Read, Write},
    net::TcpStream,
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sunbeam_common::input::InputEvent;

#[derive(Debug, Parser)]
#[command(name = "sunbeam-client")]
#[command(about = "Minimal Sunbeam remote client for LAN streaming")]
struct Cli {
    /// Remote host/IP of sunbeam-host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Host RTSP port
    #[arg(long, default_value_t = 8554)]
    rtsp_port: u16,

    /// Host control TCP port
    #[arg(long, default_value_t = 47989)]
    control_port: u16,

    /// RTSP stream path
    #[arg(long, default_value = "sunbeam")]
    stream_path: String,

    #[command(subcommand)]
    command: ClientCommand,
}

#[derive(Debug, Subcommand)]
enum ClientCommand {
    /// Start video playback via ffplay
    Play {
        /// External player binary (default: ffplay)
        #[arg(long, default_value = "ffplay")]
        player: String,
    },
    Sessions,
    Select {
        agent_id: String,
    },
    MoveMouse {
        x: i32,
        y: i32,
    },
    MouseButton {
        button: u8,
        action: ButtonAction,
    },
    Key {
        keycode: u32,
        action: ButtonAction,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ButtonAction {
    Press,
    Release,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        ClientCommand::Play { player } => {
            let rtsp_url = format!(
                "rtsp://{}:{}/{}",
                cli.host,
                cli.rtsp_port,
                cli.stream_path.trim_start_matches('/')
            );
            launch_player(&player, &rtsp_url)?;
        }
        ClientCommand::Sessions => {
            let response = send_control_command(&cli.host, cli.control_port, "sessions")?;
            println!("{response}");
        }
        ClientCommand::Select { agent_id } => {
            let response =
                send_control_command(&cli.host, cli.control_port, &format!("select {agent_id}"))?;
            println!("{response}");
        }
        ClientCommand::MoveMouse { x, y } => {
            let event = InputEvent::PointerMoveAbsolute { x, y };
            let response = send_control_command(
                &cli.host,
                cli.control_port,
                &format!("input {}", serde_json::to_string(&event)?),
            )?;
            println!("{response}");
        }
        ClientCommand::MouseButton { button, action } => {
            let event = InputEvent::PointerButton {
                button,
                pressed: matches!(action, ButtonAction::Press),
            };
            let response = send_control_command(
                &cli.host,
                cli.control_port,
                &format!("input {}", serde_json::to_string(&event)?),
            )?;
            println!("{response}");
        }
        ClientCommand::Key { keycode, action } => {
            let event = InputEvent::Key {
                keycode,
                pressed: matches!(action, ButtonAction::Press),
            };
            let response = send_control_command(
                &cli.host,
                cli.control_port,
                &format!("input {}", serde_json::to_string(&event)?),
            )?;
            println!("{response}");
        }
    }

    Ok(())
}

fn launch_player(player: &str, rtsp_url: &str) -> Result<()> {
    let status = ProcessCommand::new(player)
        .arg("-fflags")
        .arg("nobuffer")
        .arg("-flags")
        .arg("low_delay")
        .arg("-framedrop")
        .arg(rtsp_url)
        .status()
        .with_context(|| format!("launching player '{player}'"))?;

    if !status.success() {
        anyhow::bail!("player '{player}' exited with status {status}");
    }

    Ok(())
}

fn send_control_command(host: &str, control_port: u16, request: &str) -> Result<String> {
    let addr = format!("{host}:{control_port}");
    let mut stream =
        TcpStream::connect(&addr).with_context(|| format!("connecting to control tcp {addr}"))?;

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
