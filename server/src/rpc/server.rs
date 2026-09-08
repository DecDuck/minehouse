use std::sync::Arc;

use common::{
    ids::{ClientId, WorkUnitId},
    rpc::{MinehouseError, MinehouseServer},
    work::{WorkUnit, WorkUnitDone},
};
use tarpc::context::Context;

use crate::state::MinehouseState;
use crate::work::WorkUnitAction;

struct ClientSession {
    state: Arc<MinehouseState>,
    client_id: ClientId,
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        self.state.pool.release_client(&self.client_id);
    }
}

#[derive(Clone)]
pub struct MinehouseServerImpl {
    session: Arc<ClientSession>,
}

impl MinehouseServerImpl {
    // Called for each connection
    pub fn new(state: Arc<MinehouseState>) -> Self {
        Self {
            session: Arc::new(ClientSession {
                state,
                client_id: ClientId::new(),
            }),
        }
    }
}

impl MinehouseServer for MinehouseServerImpl {
    async fn poll_work(self, _context: Context) -> Vec<WorkUnit> {
        self.session.state.pool.available_work_units()
    }

    async fn claim_work(self, _context: Context, id: WorkUnitId) -> Result<(), MinehouseError> {
        self.session
            .state
            .pool
            .claim_work_unit(&id, &self.session.client_id)
    }

    async fn submit_work_unit(
        self,
        _context: Context,
        unit: WorkUnit,
    ) -> Result<bool, MinehouseError> {
        let is_done = unit.is_done();
        let unit_id = unit.id;
        self.session
            .state
            .pool
            .submit_work_unit(unit, &self.session.client_id)
            .await?;
        if is_done {
            let completed = self
                .session
                .state
                .pool
                .work_units()
                .into_iter()
                .find(|work_unit| work_unit.id == unit_id)
                .ok_or(MinehouseError::WorkUnitNotFound)?;
            completed.action(self.session.state.clone()).await?;
        }
        Ok(is_done)
    }

    async fn heartbeat(self, _context: Context) -> () {
        todo!()
    }
}
