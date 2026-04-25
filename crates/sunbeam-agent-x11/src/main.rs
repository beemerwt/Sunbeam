use anyhow::{bail, Result};
use clap::Parser;
use std::{env, fs};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "sunbeam-agent-x11")]
#[command(about = "Per-session X11 capture and input agent")]
struct Cli {
    /// Human-readable session name
    #[arg(long, default_value = "X11 Session")]
    session_name: String,

    /// Dump one synthetic BGRA frame to the given path (milestone 0 scaffold)
    #[arg(long)]
    dump_frame: Option<String>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    info!(%display, session = %cli.session_name, "starting x11 agent");

    if let Some(path) = cli.dump_frame {
        dump_synthetic_frame(&path)?;
        println!("wrote synthetic BGRA frame to {path}");
        return Ok(());
    }

    println!("sunbeam-agent-x11 initialized for DISPLAY={display}. Capture/input backends are scaffolded for future implementation.");
    Ok(())
}

fn dump_synthetic_frame(path: &str) -> Result<()> {
    let width = 320u32;
    let height = 180u32;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let b = (x * 255 / width) as u8;
            let g = (y * 255 / height) as u8;
            let r = 180u8;
            let a = 255u8;
            pixels.extend_from_slice(&[b, g, r, a]);
        }
    }

    if pixels.is_empty() {
        bail!("generated frame was empty");
    }

    fs::write(path, pixels)?;
    Ok(())
}
