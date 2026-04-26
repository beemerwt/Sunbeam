use std::{
    collections::HashMap,
    fs,
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
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

    /// Optional H.264 output file. If set, host encodes incoming BGRA frames with ffmpeg.
    #[arg(long)]
    h264_output: Option<PathBuf>,

    /// Target framerate passed to ffmpeg rawvideo ingest.
    #[arg(long, default_value_t = 30)]
    h264_fps: u32,

    /// x264 CRF for local preview encoding.
    #[arg(long, default_value_t = 23)]
    h264_crf: u32,
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
                let h264_output = cli.h264_output.clone();
                let h264_fps = cli.h264_fps;
                let h264_crf = cli.h264_crf;
                thread::spawn(move || {
                    if let Err(err) = handle_agent(
                        stream,
                        registry,
                        screenshot_dir,
                        screenshot_every_n,
                        h264_output,
                        h264_fps,
                        h264_crf,
                    ) {
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
    h264_output: Option<PathBuf>,
    h264_fps: u32,
    h264_crf: u32,
) -> Result<()> {
    let mut active_agent_id = String::new();
    let mut frame_index: u64 = 0;
    let mut encoder: Option<FfmpegEncoder> = None;

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

                if let Some(output_path) = &h264_output {
                    if descriptor.pixel_format != PixelFormat::Bgra8888 {
                        warn!(agent_id = %active_agent_id, "encoder skipping non-BGRA frame");
                        continue;
                    }

                    if encoder.is_none() {
                        encoder = Some(FfmpegEncoder::spawn(
                            output_path,
                            descriptor.width,
                            descriptor.height,
                            h264_fps,
                            h264_crf,
                        )?);
                    }

                    if let Some(enc) = encoder.as_mut() {
                        enc.write_frame(&descriptor, &payload)?;
                    }
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

    if let Some(enc) = encoder.as_mut() {
        enc.finish()?;
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

struct FfmpegEncoder {
    child: Child,
    stdin: Option<ChildStdin>,
    width: u32,
    height: u32,
}

impl FfmpegEncoder {
    fn spawn(output: &PathBuf, width: u32, height: u32, fps: u32, crf: u32) -> Result<Self> {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating h264 output directory {}", parent.display()))?;
        }

        let mut child = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("bgra")
            .arg("-s")
            .arg(format!("{}x{}", width, height))
            .arg("-r")
            .arg(fps.max(1).to_string())
            .arg("-i")
            .arg("-")
            .arg("-an")
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-tune")
            .arg("zerolatency")
            .arg("-crf")
            .arg(crf.to_string())
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("failed to spawn ffmpeg (is ffmpeg installed?)")?;

        let stdin = child
            .stdin
            .take()
            .context("ffmpeg stdin unavailable for rawvideo feed")?;
        info!(file = %output.display(), width, height, fps, "started ffmpeg encoder");

        Ok(Self {
            child,
            stdin: Some(stdin),
            width,
            height,
        })
    }

    fn write_frame(&mut self, descriptor: &FrameDescriptor, payload: &[u8]) -> Result<()> {
        if descriptor.width != self.width || descriptor.height != self.height {
            warn!(
                expected = format!("{}x{}", self.width, self.height),
                got = format!("{}x{}", descriptor.width, descriptor.height),
                "dropping frame with unexpected size for active ffmpeg stream"
            );
            return Ok(());
        }

        let expected_len = (descriptor.height as usize) * (descriptor.stride as usize);
        if payload.len() < expected_len {
            warn!(
                expected_len,
                got = payload.len(),
                "dropping short frame for ffmpeg encoder"
            );
            return Ok(());
        }

        self.stdin
            .as_mut()
            .context("ffmpeg stdin was already closed")?
            .write_all(&payload[..expected_len])
            .context("writing frame to ffmpeg stdin")?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.stdin.take() {
            stdin
                .flush()
                .context("flushing ffmpeg stdin before shutdown")?;
            drop(stdin);
        }

        let status = self.child.wait().context("waiting for ffmpeg process")?;
        if !status.success() {
            bail!("ffmpeg exited with status {status}");
        }
        Ok(())
    }
}
