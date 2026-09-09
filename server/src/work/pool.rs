use std::{sync::Arc, time::Instant};

use common::{
    ids::{ClientId, WorkUnitId},
    rpc::MinehouseError,
    work::{WorkUnit, WorkUnitDone},
};
use dashmap::DashMap;
use tokio::sync::watch;
use tracing::info;

use crate::{state::MinehouseState, work::WorkUnitLifecycle};

pub struct WorkUnitPool {
    pool: DashMap<WorkUnitId, ScheduledWorkUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkUnitState {
    Queued,
    Claimed,
    Completed,
}

#[derive(Debug, Clone)]
pub struct WorkUnitSnapshot {
    pub priority: usize,
    pub work_unit: WorkUnit,
    pub state: WorkUnitState,
}

impl WorkUnitPool {
    pub fn new() -> Self {
        Self {
            pool: DashMap::new(),
        }
    }

    pub fn queue_work_unit(&self, work_unit: WorkUnit, priority: Option<usize>) {
        let (finished, _) = watch::channel(work_unit.is_done());
        let work_unit_id = work_unit.id;
        let priority = priority.unwrap_or(0);
        if let Some(_) = self.pool.insert(
            work_unit_id,
            ScheduledWorkUnit {
                priority,
                work_unit,
                assigned_to: None,
                finished,
            },
        ) {
            panic!("UUIDv4 collision: buy a lottery ticket")
        }
        info!(?work_unit_id, priority, "work unit created");
    }

    pub fn work_units(&self) -> Vec<WorkUnit> {
        self.pool.iter().map(|v| v.work_unit.clone()).collect()
    }

    pub fn contains_work_unit(&self, id: &WorkUnitId) -> bool {
        self.pool.contains_key(id)
    }

    pub fn snapshot(&self) -> Vec<WorkUnitSnapshot> {
        self.pool
            .iter()
            .map(|scheduled| WorkUnitSnapshot {
                priority: scheduled.priority,
                state: if scheduled.work_unit.is_done() {
                    WorkUnitState::Completed
                } else if scheduled.assigned_to.is_some() {
                    WorkUnitState::Claimed
                } else {
                    WorkUnitState::Queued
                },
                work_unit: scheduled.work_unit.clone(),
            })
            .collect()
    }

    pub fn available_work_units(&self) -> Vec<WorkUnit> {
        let mut units: Vec<_> = self
            .pool
            .iter()
            .filter(|v| v.assigned_to.is_none() && !v.work_unit.is_done())
            .map(|r| r.clone())
            .collect();
        units.sort_by_key(|a| a.priority);
        let units = units.into_iter().map(|v| v.work_unit).collect::<Vec<_>>();

        units
    }

    pub fn claim_work_unit(
        &self,
        id: &WorkUnitId,
        client_id: &ClientId,
    ) -> Result<(), MinehouseError> {
        let mut scheduled_wu = self
            .pool
            .get_mut(id)
            .ok_or(MinehouseError::WorkUnitNotFound)?;
        if scheduled_wu.work_unit.is_done() {
            return Err(MinehouseError::AlreadyLocked);
        }

        match scheduled_wu.assigned_to {
            None => {
                scheduled_wu.assigned_to = Some(*client_id);
                Ok(())
            }
            Some(assigned_client) if assigned_client == *client_id => Ok(()),
            Some(_) => Err(MinehouseError::AlreadyLocked),
        }
    }

    pub async fn claim(
        &self,
        id: &WorkUnitId,
        client_id: &ClientId,
        state: Arc<MinehouseState>,
    ) -> Result<(), MinehouseError> {
        self.claim_work_unit(id, client_id)?;
        let work_unit = self
            .pool
            .get(id)
            .map(|scheduled| scheduled.work_unit.clone())
            .ok_or(MinehouseError::WorkUnitNotFound)?;
        if let Err(error) = work_unit.data.claimed(work_unit.id, state).await {
            if let Some(mut scheduled) = self.pool.get_mut(id)
                && scheduled.assigned_to == Some(*client_id)
            {
                scheduled.assigned_to = None;
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn release_client(&self, client_id: &ClientId) {
        for mut scheduled_wu in self.pool.iter_mut() {
            if scheduled_wu.assigned_to == Some(*client_id) {
                scheduled_wu.assigned_to = None;
            }
        }
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
        if scheduled_wu.work_unit.is_done() {
            return wu.is_done().then_some(()).ok_or(MinehouseError::NotLocked);
        }
        if let Some(assigned_client) = scheduled_wu.assigned_to {
            if assigned_client != *client_id {
                return Err(MinehouseError::NotYourWorkUnit);
            }
        } else {
            return Err(MinehouseError::NotLocked);
        }
        scheduled_wu.work_unit = wu;
        if scheduled_wu.work_unit.is_done() {
            scheduled_wu.assigned_to = None;
            scheduled_wu.finished.send_replace(true);
            info!(work_unit_id = ?scheduled_wu.work_unit.id, ?client_id, "work unit finished");
        }

        Ok(())
    }

    pub async fn submit(
        &self,
        work_unit: WorkUnit,
        client_id: &ClientId,
        state: Arc<MinehouseState>,
    ) -> Result<bool, MinehouseError> {
        let is_done = work_unit.is_done();
        self.submit_work_unit(work_unit.clone(), client_id).await?;
        if is_done {
            work_unit.data.done(work_unit.id, state).await?;
        } else {
            work_unit.data.updated(work_unit.id, state).await?;
        }
        Ok(is_done)
    }

    pub async fn wait_work_unit(
        &self,
        id: &WorkUnitId,
        deadline: Instant,
    ) -> Result<WorkUnit, MinehouseError> {
        let mut finished = {
            let scheduled_wu = self.pool.get(id).ok_or(MinehouseError::WorkUnitNotFound)?;
            if scheduled_wu.work_unit.is_done() {
                return Ok(scheduled_wu.work_unit.clone());
            }
            scheduled_wu.finished.subscribe()
        };

        let timeout = deadline.saturating_duration_since(Instant::now());
        tokio::time::timeout(timeout, finished.wait_for(|finished| *finished))
            .await
            .map_err(|_| MinehouseError::WorkUnitTimeout)?
            .map_err(|_| MinehouseError::WorkUnitNotFound)?;

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
    pub assigned_to: Option<ClientId>,
    /// Publishes completion to tasks waiting on this work unit.
    pub finished: watch::Sender<bool>,
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

    use super::{WorkUnitPool, WorkUnitState};

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
        pool.claim_work_unit(&id, &client_id).unwrap();
        assert_eq!(pool.pool.get(&id).unwrap().assigned_to, Some(client_id));

        let wait_pool = Arc::clone(&pool);
        let wait_task = tokio::spawn(async move {
            wait_pool
                .wait_work_unit(&id, Instant::now() + Duration::from_secs(1))
                .await
        });

        pool.submit_work_unit(work_unit(id, true), &client_id)
            .await
            .unwrap();

        let completed = wait_task.await.unwrap().unwrap();
        assert!(completed.is_done());
        assert_eq!(pool.pool.get(&id).unwrap().assigned_to, None);
        assert!(pool.available_work_units().is_empty());
    }

    #[tokio::test]
    async fn snapshot_tracks_state_and_retains_completed_work() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        let client_id = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), Some(7));

        let queued = pool.snapshot();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].priority, 7);
        assert_eq!(queued[0].state, WorkUnitState::Queued);

        pool.claim_work_unit(&id, &client_id).unwrap();
        assert_eq!(pool.snapshot()[0].state, WorkUnitState::Claimed);

        pool.submit_work_unit(work_unit(id, true), &client_id)
            .await
            .unwrap();
        let completed = pool.snapshot();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].state, WorkUnitState::Completed);
    }

    #[tokio::test]
    async fn completed_submission_can_be_retried_after_claim_release() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        let first_client = ClientId::new();
        let retry_client = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), None);
        pool.claim_work_unit(&id, &first_client).unwrap();
        pool.submit_work_unit(work_unit(id, true), &first_client)
            .await
            .unwrap();

        pool.submit_work_unit(work_unit(id, true), &retry_client)
            .await
            .unwrap();
    }

    #[test]
    fn releasing_client_makes_unfinished_work_available_again() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        let client_id = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), None);
        pool.claim_work_unit(&id, &client_id).unwrap();

        pool.release_client(&client_id);

        assert_eq!(pool.available_work_units()[0].id, id);
    }

    #[test]
    fn claim_is_idempotent_only_for_the_assigned_client() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        let client_id = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), None);

        pool.claim_work_unit(&id, &client_id).unwrap();
        pool.claim_work_unit(&id, &client_id).unwrap();

        assert!(matches!(
            pool.claim_work_unit(&id, &ClientId::new()),
            Err(MinehouseError::AlreadyLocked)
        ));
    }

    #[tokio::test]
    async fn partial_submission_keeps_the_client_claim() {
        let pool = WorkUnitPool::new();
        let id = WorkUnitId::new();
        let client_id = ClientId::new();
        pool.queue_work_unit(work_unit(id, false), None);
        pool.claim_work_unit(&id, &client_id).unwrap();

        pool.submit_work_unit(work_unit(id, false), &client_id)
            .await
            .unwrap();

        assert_eq!(pool.pool.get(&id).unwrap().assigned_to, Some(client_id));
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
