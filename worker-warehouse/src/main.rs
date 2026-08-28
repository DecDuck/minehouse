use std::time::Duration;

use bevy_ecs::component::Component;
use common::work::WorkUnitData;
use tokio::task::spawn_blocking;
use tracing::{info, warn};
use worker_core::{config::load_config, control::ControlClient};

use crate::config::WarehouseWorkerConfig;

pub mod config;
mod exec;

#[derive(Clone, Component, Default)]
pub struct WarehouseWorkerState {}

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

    let (mc_client, control_client) = config.worker.create(state, async |_client, _event, _state| {
        
    }).await?;
    let control_client = ControlClient::new(control_client);

    info!("warehouse worker ready, waiting for work");

    loop {
        let accepted = control_client
            .accept_new(|v| {
                matches!(
                    v.data,
                    WorkUnitData::IndexContainer(..) | WorkUnitData::Transfer(..)
                )
            })
            .await;

        match accepted {
            Ok(Some(unit)) => {
                let id = unit.id;
                match exec::execute(&mc_client, unit).await {
                    Ok(done) => {
                        if let Err(err) = control_client.submit(done).await {
                            warn!("failed to submit work unit {id:?}: {err:?}");
                        }
                    }
                    Err(err) => warn!("failed to execute work unit {id:?}: {err:?}"),
                }
            }
            Ok(None) => tokio::time::sleep(Duration::from_secs(2)).await,
            Err(err) => {
                warn!("failed to poll for work: {err:?}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
