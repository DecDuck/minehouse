use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use common::{
    ids::{ClientId, WorkUnitId}, rpc::{MinehouseError, MinehouseServer}, work::{WorkUnit, WorkUnitDone},
};
use tarpc::context::Context;

use crate::state::MinehouseState;

#[derive(Clone)]
pub struct MinehouseServerImpl {
    state: Arc<MinehouseState>,
    client_id: ClientId,
}

impl MinehouseServerImpl {
    // Called for each connection
    pub fn new(state: Arc<MinehouseState>) -> Self {
        Self {
            state,
            client_id: ClientId::new(),
        }
    }
}

impl MinehouseServer for MinehouseServerImpl {
    async fn poll_work(self, _context: Context) -> Vec<WorkUnit> {
        self.state.pool.available_work_units()
    }

    async fn lock_work(self, context: Context, id: WorkUnitId) -> Result<(), MinehouseError> {
        if context.deadline.duration_since(Instant::now()) < Duration::from_hours(12) {
            return Err(MinehouseError::DeadlineTooShort);
        }

        self.state.pool.lock_work_unit(&id, &self.client_id).await
    }

    async fn submit_work_unit(
        self,
        _context: Context,
        unit: WorkUnit,
    ) -> Result<bool, MinehouseError> {
        let is_done = unit.is_done();
        self.state
            .pool
            .submit_work_unit(unit, &self.client_id)
            .await?;
        Ok(is_done)
    }

    async fn heartbeat(self, _context: Context) -> () {
        todo!()
    }
}
