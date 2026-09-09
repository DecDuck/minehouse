use std::sync::Arc;

use common::{
    ids::{ClientId, WorkUnitId},
    rpc::{MinehouseError, MinehouseServer},
    work::WorkUnit,
};
use tarpc::context::Context;

use crate::state::MinehouseState;

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
            .claim(&id, &self.session.client_id, self.session.state.clone())
            .await
    }

    async fn submit_work_unit(
        self,
        _context: Context,
        unit: WorkUnit,
    ) -> Result<bool, MinehouseError> {
        self.session
            .state
            .pool
            .submit(unit, &self.session.client_id, self.session.state.clone())
            .await
    }

    async fn heartbeat(self, _context: Context) -> () {
        todo!()
    }
}
