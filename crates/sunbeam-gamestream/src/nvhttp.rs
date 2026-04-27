use anyhow::Result;
use tracing::info;

pub fn start_nvhttp_stub(bind_addr: &str) -> Result<()> {
    info!(
        bind_addr,
        "starting NVHTTP stub (TODO: implement /serverinfo,/pair,/applist,/launch,/resume,/cancel)"
    );
    Ok(())
}
