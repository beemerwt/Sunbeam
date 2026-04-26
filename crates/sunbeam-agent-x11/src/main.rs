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
    input::InputEvent,
    session::{SessionCapabilities, SessionInfo},
    transport::{read_packet, write_packet, WireMessage, WirePacket},
};
use tracing::{info, warn};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT},
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
};

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

    let display_name = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let agent_id = cli
        .agent_id
        .clone()
        .unwrap_or_else(|| format!("x11-{display_name}"));

    info!(%display_name, session = %cli.session_name, agent_id = %agent_id, "starting x11 agent");

    if let Some(path) = cli.dump_frame {
        dump_synthetic_frame(&path)?;
        println!("wrote synthetic BGRA frame to {path}");
        return Ok(());
    }

    if cli.stream_frames {
        stream_synthetic_frames(&cli, &display_name, &agent_id)?;
        return Ok(());
    }

    println!("sunbeam-agent-x11 initialized for DISPLAY={display_name}. Use --stream-frames for milestone 1 transport test.");
    Ok(())
}

fn stream_synthetic_frames(cli: &Cli, display_name: &str, agent_id: &str) -> Result<()> {
    let mut stream = UnixStream::connect(&cli.host_socket)
        .with_context(|| format!("connecting to host socket {}", cli.host_socket.display()))?;

    let session = SessionInfo {
        agent_id: agent_id.to_string(),
        backend: "x11".to_string(),
        session_name: cli.session_name.clone(),
        display: display_name.to_string(),
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

    let mut input_stream = stream
        .try_clone()
        .context("cloning host stream for input receive loop")?;
    thread::spawn(move || {
        if let Err(err) = input_receive_loop(&mut input_stream) {
            warn!(error = %err, "input receiver exited");
        }
    });

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

fn input_receive_loop(stream: &mut UnixStream) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None).context("connecting to X11 display")?;
    let root = conn.setup().roots[screen_num].root;

    loop {
        let (packet, _) = read_packet(stream)?;
        if let WireMessage::Input { event } = packet.message {
            info!(?event, "received input event from host");
            inject_input(&conn, root, &event)?;
        }
    }
}

fn inject_input(conn: &RustConnection, root: u32, event: &InputEvent) -> Result<()> {
    match event {
        InputEvent::PointerMoveAbsolute { x, y } => {
            conn.xtest_fake_input(6, 0, 0, root, *x as i16, *y as i16, 0)
                .context("injecting absolute pointer move")?;
        }
        InputEvent::PointerButton { button, pressed } => {
            let event_type = if *pressed { 4 } else { 5 };
            conn.xtest_fake_input(event_type, *button, 0, root, 0, 0, 0)
                .context("injecting pointer button")?;
        }
        InputEvent::Key { keycode, pressed } => {
            let detail = u8::try_from(*keycode).context("keycode out of range for X11")?;
            let event_type = if *pressed {
                KEY_PRESS_EVENT
            } else {
                KEY_RELEASE_EVENT
            };
            conn.xtest_fake_input(event_type, detail, 0, root, 0, 0, 0)
                .context("injecting key event")?;
        }
        InputEvent::PointerMoveRelative { .. }
        | InputEvent::Text { .. }
        | InputEvent::GamepadButton { .. }
        | InputEvent::GamepadAxis { .. } => {
            warn!(?event, "input type not yet implemented for XTest injection");
            return Ok(());
        }
    }

    conn.flush().context("flushing X11 injected input")?;
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
