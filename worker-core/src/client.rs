use std::sync::Arc;

use arc_swap::ArcSwap;
use common::{
    ids::WorkUnitId,
    rpc::{MinehouseError, MinehouseServerClient},
    work::{WorkUnit, WorkUnitDone},
};
use tarpc::{context, serde_transport, tokio_serde::formats::Bincode};
use tokio::sync::Mutex;
use tracing::info;

#[derive(Clone)]
pub struct ReconnectingMinehouseClient {
    endpoint: String,
    client: Arc<ArcSwap<MinehouseServerClient>>,
}

impl ReconnectingMinehouseClient {
    pub async fn connect(endpoint: String) -> Result<Self, anyhow::Error> {
        let client = Self::connect_once(&endpoint).await?;
        Ok(Self {
            endpoint,
            client: Arc::new(ArcSwap::from_pointee(client)),
        })
    }

    async fn connect_once(endpoint: &str) -> Result<MinehouseServerClient, anyhow::Error> {
        let mut transport = serde_transport::tcp::connect(endpoint.to_owned(), Bincode::default);
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await?;
        Ok(MinehouseServerClient::new(tarpc::client::Config::default(), transport).spawn())
    }

    async fn reconnect(&self) -> Result<(), anyhow::Error> {
        let client = Self::connect_once(&self.endpoint).await?;
        self.client.store(Arc::new(client));
        Ok(())
    }

    fn current(&self) -> Arc<MinehouseServerClient> {
        self.client.load_full()
    }

    pub async fn poll_work(&self) -> Result<Vec<WorkUnit>, anyhow::Error> {
        let result = self.current().poll_work(context::current()).await;
        match result {
            Ok(work) => Ok(work),
            Err(_error) => {
                self.reconnect().await?;
                Ok(self.current().poll_work(context::current()).await?)
            }
        }
    }

    pub async fn claim_work(
        &self,
        id: WorkUnitId,
    ) -> Result<Result<(), MinehouseError>, anyhow::Error> {
        let result = self.current().claim_work(context::current(), id).await;
        match result {
            Ok(result) => Ok(result),
            Err(_error) => {
                self.reconnect().await?;
                Ok(self.current().claim_work(context::current(), id).await?)
            }
        }
    }

    pub async fn submit_work_unit(
        &self,
        work_unit: WorkUnit,
    ) -> Result<Result<bool, MinehouseError>, anyhow::Error> {
        let result = self
            .current()
            .submit_work_unit(context::current(), work_unit.clone())
            .await;
        match result {
            Ok(result) => Ok(result),
            Err(_error) => {
                self.reconnect().await?;
                match self.claim_work(work_unit.id).await? {
                    Ok(()) => {}
                    Err(MinehouseError::AlreadyLocked) if work_unit.is_done() => {}
                    Err(error) => return Ok(Err(error)),
                }
                Ok(self
                    .current()
                    .submit_work_unit(context::current(), work_unit)
                    .await?)
            }
        }
    }
}

pub struct WorkUnitClient {
    /// tarpc client
    client: ReconnectingMinehouseClient,
    /// current wu
    work_unit: Mutex<Option<WorkUnit>>,
}

impl WorkUnitClient {
    pub fn new(client: ReconnectingMinehouseClient) -> Self {
        Self {
            client,
            work_unit: Mutex::new(None),
        }
    }

    pub async fn accept_new<F>(&self, select: F) -> Result<Option<WorkUnit>, anyhow::Error>
    where
        F: FnOnce(Vec<WorkUnit>) -> Option<WorkUnit>,
    {
        if let Some(current) = self.read().await {
            self.client.claim_work(current.id).await??;
            return Ok(Some(current));
        }

        let available = self.client.poll_work().await?;
        let Some(chosen) = select(available) else {
            return Ok(None);
        };

        self.client.claim_work(chosen.id).await??;

        let work_unit_id = chosen.id;
        let mut wu_lock = self.work_unit.lock().await;
        *wu_lock = Some(chosen);
        info!(?work_unit_id, "picked up work unit");

        Ok(wu_lock.clone())
    }

    pub async fn read(&self) -> Option<WorkUnit> {
        let wu_lock = self.work_unit.lock().await;
        wu_lock.clone()
    }

    pub async fn submit(&self, work_unit: WorkUnit) -> Result<bool, anyhow::Error> {
        let mut wu_lock = self.work_unit.lock().await;
        let Some(current) = wu_lock.as_ref() else {
            anyhow::bail!("cannot submit work unit without an active claim")
        };
        if current.id != work_unit.id {
            anyhow::bail!("cannot submit a work unit other than the currently claimed unit")
        }

        let work_unit_id = work_unit.id;
        let is_done = self.client.submit_work_unit(work_unit.clone()).await??;
        if is_done {
            *wu_lock = None;
            info!(?work_unit_id, "finished work unit");
        } else {
            *wu_lock = Some(work_unit);
            info!(?work_unit_id, "updated work unit");
        }
        Ok(is_done)
    }
}
