use bevy_ecs::component::Component;
use tokio::task::spawn_blocking;
use tracing::info;
use worker_core::{client::WorkUnitClient, config::load_config, tick::WorkerController};

use crate::{config::WarehouseWorkerConfig, imple::WarehouseWorker};

pub mod config;
pub mod imple;
pub mod index_region;

#[derive(Clone, Component, Default)]
pub struct WarehouseWorkerState {}

#[tokio::main(flavor = "local")]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("info,azalea=off")
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

    let (mc_client, control_client) = config
        .worker
        .create(state, async |client, event, state| {})
        .await?;
    let control_client = WorkUnitClient::new(control_client);

    let control_loop = WorkerController::<WarehouseWorker>::new(control_client, mc_client);

    control_loop.run().await;

    Ok(())
}
