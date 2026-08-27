use std::time::{Duration, Instant};

use common::{rpc::MinehouseServerClient, work::WorkUnit};
use tarpc::context;
use tokio::{sync::Mutex, task::JoinHandle};

pub struct ControlClient {
    /// tarpc client
    client: MinehouseServerClient,
    /// current wu
    work_unit: Mutex<Option<(WorkUnit, JoinHandle<Result<(), anyhow::Error>>)>>,
}

impl ControlClient {
    pub fn new(client: MinehouseServerClient) -> Self {
        Self {
            client,
            work_unit: Mutex::new(None),
        }
    }

    pub async fn accept_new<F>(&self, predicate: F) -> Result<Option<WorkUnit>, anyhow::Error>
    where
        F: Fn(&WorkUnit) -> bool,
    {
        let available = self.client.poll_work(context::current()).await?;
        let Some(chosen) = available.into_iter().find(predicate) else {
            return Ok(None);
        };

        let mut lock_context = context::current();
        lock_context.deadline = Instant::now() + Duration::from_hours(24);

        let client = self.client.clone();
        let work_unit_id = chosen.id;
        let lock_task_handle = tokio::spawn(async move {
            client.lock_work(lock_context, work_unit_id).await??;
            Ok(())
        });

        let mut wu_lock = self.work_unit.lock().await;
        *wu_lock = Some((chosen, lock_task_handle));

        Ok(Some(wu_lock.as_ref().unwrap().0.clone()))
    }

    pub async fn read(&self) -> Option<WorkUnit> {
        let wu_lock = self.work_unit.lock().await;
        wu_lock.as_ref().map(|v| v.0.clone())
    }
}
