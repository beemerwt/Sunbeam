use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use sunbeam_common::frame::FrameDescriptor;
use tracing::{info, warn};

use crate::types::{EncodedVideoPacket, VideoCodec};

pub struct FfmpegH264AnnexBEncoder {
    child: Child,
    stdin: Option<ChildStdin>,
    width: u32,
    height: u32,
    packet_rx: Receiver<EncodedVideoPacket>,
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl FfmpegH264AnnexBEncoder {
    pub fn spawn(width: u32, height: u32, fps: u32, crf: u32) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .arg("-loglevel")
            .arg("error")
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
            .arg("-x264-params")
            .arg("repeat-headers=1:keyint=30:min-keyint=30:scenecut=0")
            .arg("-crf")
            .arg(crf.to_string())
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-f")
            .arg("h264")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("failed to spawn ffmpeg h264 annex-b encoder")?;

        let stdin = child.stdin.take().context("ffmpeg stdin unavailable")?;
        let mut stdout = child.stdout.take().context("ffmpeg stdout unavailable")?;
        let (packet_tx, packet_rx) = mpsc::channel();

        let reader_thread = thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            let mut pending = Vec::new();
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 {
                    break;
                }
                pending.extend_from_slice(&buf[..n]);
                for packet in split_annex_b_packets(&mut pending) {
                    if packet_tx.send(packet).is_err() {
                        return;
                    }
                }
            }
        });

        info!(width, height, fps, "started ffmpeg h264 annex-b encoder");
        Ok(Self {
            child,
            stdin: Some(stdin),
            width,
            height,
            packet_rx,
            reader_thread: Some(reader_thread),
        })
    }

    pub fn write_frame(&mut self, descriptor: &FrameDescriptor, payload: &[u8]) -> Result<()> {
        if descriptor.width != self.width || descriptor.height != self.height {
            warn!("dropping frame with unexpected dimensions for active h264 encoder");
            return Ok(());
        }
        let expected_len = (descriptor.height as usize) * (descriptor.stride as usize);
        if payload.len() < expected_len {
            warn!(
                expected_len,
                got = payload.len(),
                "dropping short frame payload"
            );
            return Ok(());
        }

        self.stdin
            .as_mut()
            .context("ffmpeg stdin already closed")?
            .write_all(&payload[..expected_len])
            .context("writing raw frame to ffmpeg")?;
        Ok(())
    }

    pub fn drain_packets(&mut self) -> Vec<EncodedVideoPacket> {
        let mut packets = Vec::new();
        while let Ok(packet) = self.packet_rx.try_recv() {
            packets.push(packet);
        }
        packets
    }

    pub fn finish(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.stdin.take() {
            stdin.flush().context("flushing ffmpeg stdin")?;
            drop(stdin);
        }
        let status = self.child.wait().context("waiting for ffmpeg")?;
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
        if !status.success() {
            bail!("ffmpeg exited with status {status}");
        }
        Ok(())
    }
}

pub fn append_packets_to_file(path: &Path, packets: &[EncodedVideoPacket]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;

    for packet in packets {
        file.write_all(&packet.bytes)
            .with_context(|| format!("writing packet to {}", path.display()))?;
    }
    Ok(())
}

fn split_annex_b_packets(buffer: &mut Vec<u8>) -> Vec<EncodedVideoPacket> {
    let mut out = Vec::new();
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 < buffer.len() {
        if buffer[i..].starts_with(&[0, 0, 1]) {
            starts.push((i, 3));
            i += 3;
        } else if i + 4 < buffer.len() && buffer[i..].starts_with(&[0, 0, 0, 1]) {
            starts.push((i, 4));
            i += 4;
        } else {
            i += 1;
        }
    }

    if starts.len() < 2 {
        return out;
    }

    for window in starts.windows(2) {
        let (start, prefix_len) = window[0];
        let (end, _) = window[1];
        let nal_header_index = start + prefix_len;
        let nal_type = buffer.get(nal_header_index).map(|b| b & 0x1f).unwrap_or(0);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or_default();
        out.push(EncodedVideoPacket {
            codec: VideoCodec::H264,
            timestamp_micros: ts,
            keyframe: nal_type == 5,
            bytes: buffer[start..end].to_vec(),
        });
    }

    let (last_start, _) = starts[starts.len() - 1];
    buffer.drain(..last_start);
    out
}
