use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{ids::WorkUnitId, work::WorkUnit};

#[derive(Serialize, Deserialize, Error, Debug)]
pub enum MinehouseError {
    #[error("work unit not found")]
    WorkUnitNotFound,
    #[error("work unit already locked")]
    AlreadyLocked,
    #[error("work  unit not locked")]
    NotLocked,
    #[error("deadline too short")]
    DeadlineTooShort,
    #[error("not your work unit")]
    NotYourWorkUnit,
    #[error("timed out waiting for work unit")]
    WorkUnitTimeout,
    #[error("sqlx error: {0}")]
    SqlxError(String),
}

impl From<sqlx::Error> for MinehouseError {
    fn from(value: sqlx::Error) -> Self {
        Self::SqlxError(format!("{:?}", value))
    }
}

#[tarpc::service]
pub trait MinehouseServer {
    /// Polls for more work to do
    async fn poll_work() -> Vec<WorkUnit>;
    /// Locks a workunit for only this client work on. Long-polling, will only return once work is done or lock is lost
    async fn lock_work(id: WorkUnitId) -> Result<(), MinehouseError>;
    /// Submits a workunit. Returns false if there's still more to do
    async fn submit_work_unit(unit: WorkUnit) -> Result<bool, MinehouseError>;
    /// Heartbeats the client
    async fn heartbeat();
}
