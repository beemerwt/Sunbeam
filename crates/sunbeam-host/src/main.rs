use anyhow::Result;
use clap::Parser;
use sunbeam_common::session::{SessionCapabilities, SessionInfo};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "sunbeam-host")]
#[command(about = "Global session-oriented streaming host")]
struct Cli {
    /// Listen address for future client/control protocol
    #[arg(long, default_value = "127.0.0.1:47989")]
    bind: String,

    /// Print a sample session table and exit
    #[arg(long)]
    list_sample_sessions: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    info!(bind = %cli.bind, "starting sunbeam-host");

    if cli.list_sample_sessions {
        for session in sample_sessions() {
            println!(
                "{}\t{}\t{}\t{}\t{}x{}",
                session.agent_id,
                session.backend,
                session.session_name,
                session.display,
                session.width,
                session.height
            );
        }
        return Ok(());
    }

    println!("sunbeam-host initialized. Networking/encoding pipeline is scaffolded for future implementation.");
    Ok(())
}

fn sample_sessions() -> Vec<SessionInfo> {
    vec![
        SessionInfo {
            agent_id: "x11-:0".into(),
            backend: "x11".into(),
            session_name: "Local Desktop".into(),
            display: ":0".into(),
            width: 2560,
            height: 1440,
            refresh_hz: 60,
            capabilities: SessionCapabilities {
                capture_root: true,
                capture_window: false,
                inject_keyboard_mouse: true,
                inject_gamepad: false,
            },
        },
        SessionInfo {
            agent_id: "x11-:1".into(),
            backend: "x11".into(),
            session_name: "Media Desktop".into(),
            display: ":1".into(),
            width: 1920,
            height: 1080,
            refresh_hz: 60,
            capabilities: SessionCapabilities {
                capture_root: true,
                capture_window: false,
                inject_keyboard_mouse: true,
                inject_gamepad: false,
            },
        },
    ]
}
