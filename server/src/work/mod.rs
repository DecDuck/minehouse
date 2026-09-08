pub mod craft;
pub mod pool;

use std::{future::Future, sync::Arc};

use crate::state::MinehouseState;
use common::work::{WorkUnit, WorkUnitData};

pub trait WorkUnitAction {
    fn action(
        self,
        state: Arc<MinehouseState>,
    ) -> impl Future<Output = Result<(), common::rpc::MinehouseError>> + Send;
}

impl WorkUnitAction for WorkUnitData {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), common::rpc::MinehouseError> {
        match self {
            WorkUnitData::Craft(craft) => craft.action(state).await,
            WorkUnitData::IndexRegion(_)
            | WorkUnitData::IndexContainer(_)
            | WorkUnitData::Transfer(_) => Ok(()),
        }
    }
}

impl WorkUnitAction for WorkUnit {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), common::rpc::MinehouseError> {
        self.data.action(state).await
    }
}
