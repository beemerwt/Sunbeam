use std::{
    env, fs,
    os::unix::net::UnixStream,
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use sunbeam_common::{
    frame::{FrameDescriptor, PixelFormat},
    session::{SessionCapabilities, SessionInfo},
    transport::{write_packet, WireMessage, WirePacket},
};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "sunbeam-agent-x11")]
#[command(about = "Per-session X11 capture and input agent")]
struct Cli {
    /// Human-readable session name
    #[arg(long, default_value = "X11 Session")]
    session_name: String,

    /// Agent identifier reported to host
    #[arg(long)]
    agent_id: Option<String>,

    /// Host unix socket path
    #[arg(long, default_value = "/tmp/sunbeam.sock")]
    host_socket: PathBuf,

    /// Dump one synthetic BGRA frame to the given path (milestone 0 scaffold)
    #[arg(long)]
    dump_frame: Option<String>,

    /// Stream synthetic frames to host (milestone 1 scaffold)
    #[arg(long)]
    stream_frames: bool,

    /// Streaming FPS for synthetic mode
    #[arg(long, default_value_t = 5)]
    fps: u32,

    /// Number of frames to stream before exit
    #[arg(long, default_value_t = 120)]
    frame_count: u64,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let agent_id = cli.agent_id.unwrap_or_else(|| format!("x11-{display}"));

    info!(%display, session = %cli.session_name, agent_id = %agent_id, "starting x11 agent");

    if let Some(path) = cli.dump_frame {
        dump_synthetic_frame(&path)?;
        println!("wrote synthetic BGRA frame to {path}");
        return Ok(());
    }

    if cli.stream_frames {
        stream_synthetic_frames(&cli, &display, &agent_id)?;
        return Ok(());
    }

    println!("sunbeam-agent-x11 initialized for DISPLAY={display}. Use --stream-frames for milestone 1 transport test.");
    Ok(())
}

fn stream_synthetic_frames(cli: &Cli, display: &str, agent_id: &str) -> Result<()> {
    let mut stream = UnixStream::connect(&cli.host_socket)
        .with_context(|| format!("connecting to host socket {}", cli.host_socket.display()))?;

    let session = SessionInfo {
        agent_id: agent_id.to_string(),
        backend: "x11".to_string(),
        session_name: cli.session_name.clone(),
        display: display.to_string(),
        width: 1280,
        height: 720,
        refresh_hz: cli.fps,
        capabilities: SessionCapabilities {
            capture_root: true,
            capture_window: false,
            inject_keyboard_mouse: true,
            inject_gamepad: false,
        },
    };

    write_packet(
        &mut stream,
        &WirePacket {
            message: WireMessage::Register { session },
        },
        None,
    )?;

    let frame_interval = if cli.fps == 0 {
        Duration::from_millis(200)
    } else {
        Duration::from_millis((1000 / cli.fps) as u64)
    };

    for frame_id in 0..cli.frame_count {
        let (descriptor, payload) = build_synthetic_frame(frame_id, 1280, 720)?;
        write_packet(
            &mut stream,
            &WirePacket {
                message: WireMessage::Frame {
                    payload_len: payload.len() as u32,
                    descriptor,
                },
            },
            Some(&payload),
        )?;

        thread::sleep(frame_interval);
    }

    Ok(())
}

fn build_synthetic_frame(
    frame_id: u64,
    width: u32,
    height: u32,
) -> Result<(FrameDescriptor, Vec<u8>)> {
    let stride = width * 4;
    let mut pixels = vec![0u8; (stride * height) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * stride + (x * 4)) as usize;
            let moving = ((x + frame_id as u32) % width) * 255 / width;
            pixels[idx] = moving as u8;
            pixels[idx + 1] = ((y * 255) / height) as u8;
            pixels[idx + 2] = 170;
            pixels[idx + 3] = 255;
        }
    }

    if pixels.is_empty() {
        bail!("generated frame was empty");
    }

    let timestamp_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX_EPOCH")?
        .as_nanos() as u64;

    Ok((
        FrameDescriptor {
            frame_id,
            width,
            height,
            stride,
            pixel_format: PixelFormat::Bgra8888,
            timestamp_ns,
        },
        pixels,
    ))
}

fn dump_synthetic_frame(path: &str) -> Result<()> {
    let (_descriptor, pixels) = build_synthetic_frame(0, 320, 180)?;
    fs::write(path, pixels)?;
    Ok(())
}
