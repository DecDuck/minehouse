use std::sync::Arc;

use axum::Router;
use oasgen::Server;
use tokio::net::TcpListener;
use tracing::info;

use crate::{config::load_config, db::DatabaseHandle, state::MinehouseState};

pub mod api;
pub mod config;
pub mod db;
pub mod rpc;
pub mod state;
pub mod work;

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

    let app_state = MinehouseState::new(db_handle);

    let app = Router::new()
        .merge(server.into_router())
        // websockets
        .with_state(app_state);

    let listener = TcpListener::bind(config.bind_addr.clone()).await?;
    info!("listening on {:?}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
