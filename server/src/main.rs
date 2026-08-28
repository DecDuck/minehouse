use std::{sync::Arc, usize};

use axum::Router;
use common::rpc::MinehouseServer as _;
use futures::{StreamExt as _, future};
use oasgen::Server;
use tarpc::{
    serde_transport,
    server::{BaseChannel, Channel as _},
};
use tokio::{join, net::TcpListener};
use tracing::info;

use crate::{
    config::load_config, db::DatabaseHandle, planner::PlannerQueue,
    rpc::server::MinehouseServerImpl, state::MinehouseState, work::pool::WorkUnitPool,
};

pub mod api;
pub mod config;
pub mod db;
pub mod rpc;
pub mod state;
pub mod work;
pub mod mwms;
pub mod planner;
pub mod region;

#[tokio::main]
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

    let config = load_config().await?;

    let mut server = Server::axum();
    server.openapi.info.title = format!("Minehouse");

    let server = server
        // Add routes here
        // End routes here
        ;

    #[cfg(debug_assertions)]
    let server = server.route_yaml_spec("/api/v1/openapi.yaml");
    #[cfg(debug_assertions)]
    let server = server.swagger_ui("/_swagger/");
    #[cfg(debug_assertions)]
    let server = server.write_and_exit_if_env_var_set(env!("OPENAPI_OUTPUT"));

    let server = server.freeze();

    let db_handle = Arc::new(DatabaseHandle::new(&config).await?);
    info!("connected to database");

    let work_unit_pool = Arc::new(WorkUnitPool::new());
    let planner_queue = Arc::new(PlannerQueue::new());

    let app_state = Arc::new(MinehouseState::new(db_handle, work_unit_pool, planner_queue));
    let server_app_state = app_state.clone();

    let planner_state = app_state.clone();
    tokio::spawn(async move { planner::run(planner_state).await });

    let scheduler_state = app_state.clone();
    let reconcile_period = std::time::Duration::from_secs(config.reconcile_interval_secs);
    tokio::spawn(async move { planner::schedule::run(scheduler_state, reconcile_period).await });

    let app = Router::new()
        .merge(server.into_router())
        .merge(api::router())
        // websockets
        .with_state(app_state);

    let axum_listener = TcpListener::bind(config.api_bind_addr.clone()).await?;
    info!("api listening on {:?}", axum_listener.local_addr()?);
    let axum_future = axum::serve(axum_listener, app);

    let control_listener = TcpListener::bind(config.control_bind_addr.clone()).await?;
    info!("control listening on {:?}", control_listener.local_addr()?);
    let mut control_listener = serde_transport::tcp::listen_on(
        control_listener,
        tarpc::tokio_serde::formats::Bincode::default,
    )
    .await?;
    control_listener.config_mut().max_frame_length(usize::MAX);

    let control_future = control_listener
        .filter_map(|r| future::ready(r.ok()))
        .map(BaseChannel::with_defaults)
        .for_each_concurrent(None, |channel| {
            let app_state = server_app_state.clone();
            async move {
                let server = MinehouseServerImpl::new(app_state);
                channel
                    .execute(server.serve())
                    .for_each_concurrent(None, |response| async move {
                        tokio::spawn(response);
                    })
                    .await;
            }
        });

    let (axum_error, _) = join!(axum_future, control_future);
    axum_error?;

    Ok(())
}
