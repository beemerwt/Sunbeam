use std::{
    collections::HashMap,
    fs,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use clap::Parser;
use image::{ImageBuffer, Rgba};
use sunbeam_common::{
    frame::{FrameDescriptor, PixelFormat},
    session::SessionInfo,
    transport::{read_packet, WireMessage},
};
use tracing::{error, info, warn};

#[derive(Debug, Parser)]
#[command(name = "sunbeam-host")]
#[command(about = "Global session-oriented streaming host")]
struct Cli {
    /// Listen address placeholder for future remote clients
    #[arg(long, default_value = "127.0.0.1:47989")]
    bind: String,

    /// Unix socket path used by session agents
    #[arg(long, default_value = "/tmp/sunbeam.sock")]
    socket_path: PathBuf,

    /// Directory where host writes screenshot PNG files
    #[arg(long, default_value = "./artifacts/screenshots")]
    screenshot_dir: PathBuf,

    /// Store every Nth frame from each agent
    #[arg(long, default_value_t = 30)]
    screenshot_every_n: u64,
}

#[derive(Debug, Default)]
struct Registry {
    sessions: HashMap<String, SessionInfo>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    if let Some(parent) = cli.socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating socket parent {}", parent.display()))?;
    }
    fs::create_dir_all(&cli.screenshot_dir)
        .with_context(|| format!("creating screenshot dir {}", cli.screenshot_dir.display()))?;

    if cli.socket_path.exists() {
        fs::remove_file(&cli.socket_path)
            .with_context(|| format!("removing stale socket {}", cli.socket_path.display()))?;
    }

    let listener = UnixListener::bind(&cli.socket_path)
        .with_context(|| format!("binding {}", cli.socket_path.display()))?;

    info!(bind = %cli.bind, socket = %cli.socket_path.display(), "sunbeam-host listening");

    let registry = Arc::new(Mutex::new(Registry::default()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let registry = Arc::clone(&registry);
                let screenshot_dir = cli.screenshot_dir.clone();
                let screenshot_every_n = cli.screenshot_every_n;
                thread::spawn(move || {
                    if let Err(err) =
                        handle_agent(stream, registry, screenshot_dir, screenshot_every_n)
                    {
                        error!(error = %err, "agent handler exited with error");
                    }
                });
            }
            Err(err) => {
                warn!(error = %err, "failed to accept agent connection");
            }
        }
    }

    Ok(())
}

fn handle_agent(
    mut stream: UnixStream,
    registry: Arc<Mutex<Registry>>,
    screenshot_dir: PathBuf,
    screenshot_every_n: u64,
) -> Result<()> {
    let mut active_agent_id = String::new();
    let mut frame_index: u64 = 0;

    loop {
        let (packet, payload) = match read_packet(&mut stream) {
            Ok(packet) => packet,
            Err(err) => {
                if !active_agent_id.is_empty() {
                    info!(agent_id = %active_agent_id, error = %err, "agent disconnected");
                }
                break;
            }
        };

        match packet.message {
            WireMessage::Register { session } => {
                active_agent_id = session.agent_id.clone();
                register_session(&registry, session)?;
            }
            WireMessage::Frame {
                descriptor,
                payload_len: _,
            } => {
                frame_index = frame_index.saturating_add(1);
                if frame_index % screenshot_every_n == 0 {
                    save_png(&screenshot_dir, &active_agent_id, &descriptor, &payload)?;
                }
            }
            WireMessage::Heartbeat => {
                info!(agent_id = %active_agent_id, "heartbeat");
            }
        }
    }

    if !active_agent_id.is_empty() {
        let mut registry = registry.lock().expect("registry lock poisoned");
        registry.sessions.remove(&active_agent_id);
        info!(agent_id = %active_agent_id, "removed disconnected session");
    }

    Ok(())
}

fn register_session(registry: &Arc<Mutex<Registry>>, session: SessionInfo) -> Result<()> {
    let mut registry = registry.lock().expect("registry lock poisoned");
    registry
        .sessions
        .insert(session.agent_id.clone(), session.clone());

    println!("ID\tBACKEND\tNAME\tDISPLAY\tRESOLUTION");
    for s in registry.sessions.values() {
        println!(
            "{}\t{}\t{}\t{}\t{}x{}",
            s.agent_id, s.backend, s.session_name, s.display, s.width, s.height
        );
    }
    info!(agent_id = %session.agent_id, "registered agent session");
    Ok(())
}

fn save_png(
    screenshot_dir: &PathBuf,
    agent_id: &str,
    descriptor: &FrameDescriptor,
    payload: &[u8],
) -> Result<()> {
    if descriptor.pixel_format != PixelFormat::Bgra8888 {
        warn!(agent_id = %agent_id, "skipping non-BGRA frame");
        return Ok(());
    }

    let expected_len = (descriptor.height as usize) * (descriptor.stride as usize);
    if payload.len() < expected_len {
        warn!(
            agent_id = %agent_id,
            expected_len,
            got = payload.len(),
            "skipping short frame payload"
        );
        return Ok(());
    }

    let mut rgba = Vec::with_capacity((descriptor.width * descriptor.height * 4) as usize);
    for row in 0..descriptor.height as usize {
        let row_start = row * descriptor.stride as usize;
        let row_bytes = &payload[row_start..row_start + (descriptor.width as usize * 4)];

        for px in row_bytes.chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }

    let image: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_vec(descriptor.width, descriptor.height, rgba)
            .context("failed to build image buffer from frame")?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX_EPOCH")?
        .as_millis();

    let safe_agent = agent_id.replace(':', "_");
    let file = screenshot_dir.join(format!("{safe_agent}_{ts}.png"));
    image
        .save(&file)
        .with_context(|| format!("saving {}", file.display()))?;

    info!(agent_id = %agent_id, file = %file.display(), "saved screenshot");
    Ok(())
}
