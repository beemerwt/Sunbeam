use anyhow::Result;
use tracing::info;

pub fn start_rtsp_stub(bind_addr: &str) -> Result<()> {
    info!(
        bind_addr,
        "starting RTSP stub (TODO: implement OPTIONS/DESCRIBE/SETUP/PLAY/TEARDOWN)"
    );
    Ok(())
}
