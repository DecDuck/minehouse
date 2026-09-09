pub mod craft;
pub mod pool;
pub mod transfer;

use std::{future::Future, sync::Arc};

use crate::state::MinehouseState;
use common::{ids::WorkUnitId, work::WorkUnitData};

pub trait WorkUnitLifecycle {
    fn claimed(
        &self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> impl Future<Output = Result<(), common::rpc::MinehouseError>> + Send;

    fn updated(
        self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> impl Future<Output = Result<(), common::rpc::MinehouseError>> + Send;

    fn done(
        self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> impl Future<Output = Result<(), common::rpc::MinehouseError>> + Send;
}

impl WorkUnitLifecycle for WorkUnitData {
    async fn claimed(
        &self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> Result<(), common::rpc::MinehouseError> {
        match self {
            WorkUnitData::Craft(work) => work.claimed(id, state).await,
            WorkUnitData::Transfer(work) => work.claimed(id, state).await,
            WorkUnitData::IndexRegion(_) | WorkUnitData::IndexContainer(_) => Ok(()),
        }
    }

    async fn updated(
        self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> Result<(), common::rpc::MinehouseError> {
        match self {
            WorkUnitData::Craft(work) => work.updated(id, state).await,
            WorkUnitData::Transfer(work) => work.updated(id, state).await,
            WorkUnitData::IndexRegion(_) | WorkUnitData::IndexContainer(_) => Ok(()),
        }
    }

    async fn done(
        self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> Result<(), common::rpc::MinehouseError> {
        match self {
            WorkUnitData::Craft(work) => work.done(id, state).await,
            WorkUnitData::Transfer(work) => work.done(id, state).await,
            WorkUnitData::IndexRegion(_) | WorkUnitData::IndexContainer(_) => Ok(()),
        }
    }
}
