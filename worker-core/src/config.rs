use std::future::Future;

use azalea::{Client, Event, ecs::component::Component};
use serde::Deserialize;

use crate::{account::Account, client::ReconnectingMinehouseClient};

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
    ) -> Result<(Client, ReconnectingMinehouseClient), anyhow::Error>
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

        let server_client = ReconnectingMinehouseClient::connect(self.control_endpoint).await?;

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
