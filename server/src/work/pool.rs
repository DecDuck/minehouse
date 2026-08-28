use std::{sync::Arc, time::Instant};

use common::{
    ids::{ClientId, WorkUnitId}, rpc::MinehouseError, sync::DropLockAndNotify, work::{WorkUnit, WorkUnitDone},
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

    /// Queues a work unit. When `dedup_key` is set, the unit is skipped (returns
    /// `false`) if a live, not-yet-done unit with the same key is already queued.
    pub fn queue_work_unit(
        &self,
        work_unit: WorkUnit,
        priority: Option<usize>,
        dedup_key: Option<String>,
    ) -> bool {
        if let Some(key) = dedup_key.as_deref() {
            let already_queued = self
                .pool
                .iter()
                .any(|v| v.dedup_key.as_deref() == Some(key) && !v.work_unit.is_done());
            if already_queued {
                return false;
            }
        }

        if self
            .pool
            .insert(
                work_unit.id,
                ScheduledWorkUnit {
                    priority: priority.unwrap_or(0),
                    work_unit,
                    assigned_to: Arc::new(DropLockAndNotify::new()),
                    dedup_key,
                },
            )
            .is_some()
        {
            panic!("UUIDv4 collision: buy a lottery ticket")
        }

        true
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
        // Clone out the notifier and drop the DashMap ref before the long await,
        // otherwise the shard lock is held for the entire lock lifetime and
        // `submit_work_unit` on the same key would deadlock.
        let assigned_to = {
            let wu = self.pool.get(id).ok_or(MinehouseError::WorkUnitNotFound)?;
            wu.assigned_to.clone()
        };
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
        let is_done = wu.is_done();
        scheduled_wu.work_unit = wu;
        if is_done {
            // Wake the long-poll in `lock_work_unit` now the unit is finished.
            scheduled_wu.assigned_to.mark_finished();
        }

        Ok(())
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
    /// Logical key used to skip enqueuing duplicate work.
    pub dedup_key: Option<String>,
}
