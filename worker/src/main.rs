use tokio::task::spawn_blocking;

use crate::config::load_config;

pub mod config;
pub mod drasl;

// Current thread runtime due to azalea LocalSet requirement
// Makes it easier
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let config = spawn_blocking(|| load_config()).await??;


    Ok(())
}
