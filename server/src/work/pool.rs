use std::{sync::Arc, time::Instant};

use common::{
    ids::{ClientId, WorkUnitId},
    rpc::MinehouseError,
    sync::DropLockAndNotify,
    work::{WorkUnit, WorkUnitDone},
};
use dashmap::DashMap;

pub struct WorkUnitPool {
    pool: DashMap<WorkUnitId, ScheduledWorkUnit>,
}

impl WorkUnitPool {
    pub fn new() -> Self {
        Self {
            pool: DashMap::new(),
        }
    }

    pub fn queue_work_unit(&self, work_unit: WorkUnit, priority: Option<usize>) {
        if let Some(_) = self.pool.insert(
            work_unit.id,
            ScheduledWorkUnit {
                priority: priority.unwrap_or(0),
                work_unit,
                assigned_to: Arc::new(DropLockAndNotify::new()),
            },
        ) {
            panic!("UUIDv4 collision: buy a lottery ticket")
        }
    }

    pub fn work_units(&self) -> Vec<WorkUnit> {
        self.pool.iter().map(|v| v.work_unit.clone()).collect()
    }

    pub fn available_work_units(&self) -> Vec<WorkUnit> {
        let mut units: Vec<_> = self
            .pool
            .iter()
            .filter(|v| v.assigned_to.read().is_none() && !v.work_unit.is_done())
            .map(|r| r.clone())
            .collect();
        units.sort_by_key(|a| a.priority);
        let units = units.into_iter().map(|v| v.work_unit).collect::<Vec<_>>();

        units
    }

    pub async fn lock_work_unit(
        &self,
        id: &WorkUnitId,
        client_id: &ClientId,
    ) -> Result<(), MinehouseError> {
        let assigned_to = self
            .pool
            .get(id)
            .map(|wu| Arc::clone(&wu.assigned_to))
            .ok_or(MinehouseError::WorkUnitNotFound)?;
        let Some(guard) = assigned_to.lock(*client_id).await else {
            return Err(MinehouseError::AlreadyLocked);
        };

        guard.wait_finished().await;
        Ok(())
    }

    pub async fn submit_work_unit(
        &self,
        wu: WorkUnit,
        client_id: &ClientId,
    ) -> Result<(), MinehouseError> {
        let mut scheduled_wu = self
            .pool
            .get_mut(&wu.id)
            .ok_or(MinehouseError::WorkUnitNotFound)?;
        if let Some(assigned_client) = scheduled_wu.assigned_to.read() {
            if assigned_client != *client_id {
                return Err(MinehouseError::NotYourWorkUnit);
            }
        } else {
            return Err(MinehouseError::NotLocked);
        }
        scheduled_wu.work_unit = wu;
        if scheduled_wu.work_unit.is_done() {
            scheduled_wu.assigned_to.mark_finished();
        }

        Ok(())
    }

    pub async fn wait_work_unit(
        &self,
        id: &WorkUnitId,
        deadline: Instant,
    ) -> Result<WorkUnit, MinehouseError> {
        let assigned_to = {
            let scheduled_wu = self.pool.get(id).ok_or(MinehouseError::WorkUnitNotFound)?;
            if scheduled_wu.work_unit.is_done() {
                return Ok(scheduled_wu.work_unit.clone());
            }
            Arc::clone(&scheduled_wu.assigned_to)
        };

        let timeout = deadline.saturating_duration_since(Instant::now());
        tokio::time::timeout(timeout, assigned_to.wait_finished())
            .await
            .map_err(|_| MinehouseError::WorkUnitTimeout)?;

        self.pool
            .get(id)
            .map(|scheduled_wu| scheduled_wu.work_unit.clone())
            .ok_or(MinehouseError::WorkUnitNotFound)
    }
}

#[derive(Clone)]
// Internal to this module
struct ScheduledWorkUnit {
    /// Priority of this work unit. Highest priority wins
    pub priority: usize,
    /// Actual work unit
    pub work_unit: WorkUnit,
    /// Worker that is workunit is assigned to
    pub assigned_to: Arc<DropLockAndNotify<ClientId>>,
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use common::{
        ids::{ClientId, WorkUnitId},
        rpc::MinehouseError,
        vec::Point,
        work::{WorkUnit, WorkUnitData, WorkUnitDone},
        work_units::index_container::IndexContainerWorkUnit,
    };
    use uuid::Uuid;

    use super::WorkUnitPool;

    fn work_unit(id: WorkUnitId, done: bool) -> WorkUnit {
        WorkUnit {
            id,
            data: WorkUnitData::IndexContainer(IndexContainerWorkUnit {
                position: Point {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                container_id: Uuid::new_v4(),
                output: if done { Some(Vec::new()) } else { None },
            }),
        }
    }

    #[tokio::test]
    async fn available_work_units_are_sorted_and_exclude_done_units() {
        let pool = WorkUnitPool::new();
        let first = work_unit(WorkUnitId::new(), false);
        let done = work_unit(WorkUnitId::new(), true);
        let last = work_unit(WorkUnitId::new(), false);

        pool.queue_work_unit(last.clone(), Some(10));
        pool.queue_work_unit(done, Some(1));
        pool.queue_work_unit(first.clone(), Some(0));

        assert_eq!(
            pool.available_work_units()
                .iter()
                .map(|unit| unit.id)
                .collect::<Vec<_>>(),
            vec![first.id, last.id]
        );
    }

    #[tokio::test]
    async fn submit_of_done_work_unit_finishes_lock_and_waiters() {
        let pool = Arc::new(WorkUnitPool::new());
        let id = WorkUnitId::new();
        let client_id = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), None);

        let lock_pool = Arc::clone(&pool);
        let lock_client = client_id;
        let lock_task =
            tokio::spawn(async move { lock_pool.lock_work_unit(&id, &lock_client).await });

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if pool.pool.get(&id).and_then(|unit| unit.assigned_to.read()) == Some(client_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let wait_pool = Arc::clone(&pool);
        let wait_task = tokio::spawn(async move {
            wait_pool
                .wait_work_unit(&id, Instant::now() + Duration::from_secs(1))
                .await
        });

        pool.submit_work_unit(work_unit(id, true), &client_id)
            .await
            .unwrap();

        lock_task.await.unwrap().unwrap();
        let completed = wait_task.await.unwrap().unwrap();
        assert!(completed.is_done());
        assert!(pool.available_work_units().is_empty());
    }

    #[tokio::test]
    async fn waiting_for_an_incomplete_work_unit_times_out() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        pool.queue_work_unit(work_unit(id, false), None);

        let result = pool
            .wait_work_unit(&id, Instant::now() + Duration::from_millis(10))
            .await;

        assert!(matches!(result, Err(MinehouseError::WorkUnitTimeout)));
    }
}
