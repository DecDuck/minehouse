use std::{marker::PhantomData, time::Duration};

use azalea::Client;
use common::work::WorkUnit;
use tokio::time::{self, MissedTickBehavior};
use tracing::error;

use crate::client::WorkUnitClient;

const TICK_INTERVAL: Duration = Duration::from_millis(20);

pub trait WorkerControlLoop {
    fn can_accept(wu: &WorkUnit) -> bool;
    fn work(
        wu: WorkUnit,
        client: &WorkUnitClient,
        mc_client: &Client,
    ) -> impl Future<Output = Result<(), anyhow::Error>>;
}

pub struct WorkerController<T>
where
    T: WorkerControlLoop,
{
    controller: PhantomData<T>,
    client: WorkUnitClient,
    mc_client: Client,
}

impl<T> WorkerController<T>
where
    T: WorkerControlLoop,
{
    pub fn new(client: WorkUnitClient, mc_client: Client) -> Self {
        Self {
            controller: PhantomData,
            client,
            mc_client,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            if let Err(error) = self.tick().await {
                error!(error = ?error, "worker tick failed");
            }
        }
    }

    async fn tick(&self) -> Result<(), anyhow::Error> {
        let Some(work_unit) = self.client.accept_new(T::can_accept).await? else {
            return Ok(());
        };

        T::work(work_unit, &self.client, &self.mc_client).await
    }
}
