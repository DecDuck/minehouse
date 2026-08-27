use bevy_ecs::component::Component;
use common::work::{WorkUnit, WorkUnitData};
use tokio::task::spawn_blocking;
use tracing::info;
use worker_core::{config::load_config, control::ControlClient};

use crate::config::WarehouseWorkerConfig;

pub mod config;

#[derive(Clone, Component, Default)]
pub struct WarehouseWorkerState {

}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        // Use a more compact, abbreviated log format
        .compact()
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(false)
        // Build the subscriber
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("warehouse worker starting...");

    let config: WarehouseWorkerConfig = spawn_blocking(|| load_config()).await??;

    let state = WarehouseWorkerState {};

    let (mc_client, control_client) = config.worker.create(state, async |client, event, state| {
        
    }).await?;
    let control_client = ControlClient::new(control_client);

    let wu = control_client.accept_new(|v| matches!(v.data, WorkUnitData::IndexContainer(..))).await?;

    Ok(())
}
