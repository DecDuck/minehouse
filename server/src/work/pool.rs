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
        let wu = self
            .pool
            .get(id)
            .ok_or(MinehouseError::WorkUnitNotFound)?;
        let Some(guard) = wu.assigned_to.lock(*client_id).await else {
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
}
