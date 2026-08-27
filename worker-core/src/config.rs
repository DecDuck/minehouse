use std::future::Future;

use azalea::{Client, Event, ecs::component::Component};
use common::rpc::MinehouseServerClient;
use serde::Deserialize;
use tarpc::{client, serde_transport, tokio_serde::formats::Bincode};

use crate::account::Account;

#[derive(Deserialize)]
pub struct WorkerConfig {
    pub control_endpoint: String,
    pub server_addr: String,
    pub account: Account,
}

impl WorkerConfig {
    pub async fn create<S, H, Fut, R>(
        self,
        state: S,
        handler: H,
    ) -> Result<(Client, MinehouseServerClient), anyhow::Error>
    where
        S: Default + Send + Sync + Clone + Component + 'static,
        H: Fn(Client, Event, S) -> Fut + Send + 'static,
        Fut: Future<Output = R> + Send + 'static,
    {
        let account = self.account.signin().await?;
        let (client, mut events) = Client::join(account, self.server_addr).await?;

        client
            .ecs
            .write()
            .entity_mut(client.entity)
            .insert(state.clone());

        let event_client = client.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                handler(event_client.clone(), event, state.clone()).await;
            }
        });

        let mut transport = serde_transport::tcp::connect(self.control_endpoint, Bincode::default);
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await?;
        let server_client =
            MinehouseServerClient::new(client::Config::default(), transport).spawn();

        Ok((client, server_client))
    }
}

pub fn load_config<T>() -> Result<T, anyhow::Error>
where
    T: for<'a> Deserialize<'a>,
{
    let config_data = std::fs::read_to_string("./config.yaml")?;
    let config = serde_yaml2::from_str(&config_data)?;
    Ok(config)
}
