use std::sync::Arc;

use common::{
    ids::WorkUnitId,
    rpc::MinehouseError,
    work::{WorkUnit, WorkUnitData},
    work_units::transfer::TransferWorkUnit,
};

use super::WorkUnitLifecycle;
use crate::state::MinehouseState;

async fn apply_snapshot(
    work: &TransferWorkUnit,
    state: &MinehouseState,
) -> Result<(), MinehouseError> {
    let from_contents = work.from_contents.as_ref().ok_or_else(|| {
        MinehouseError::SqlxError("completed transfer has no source snapshot".into())
    })?;
    let to_contents = work.to_contents.as_ref().ok_or_else(|| {
        MinehouseError::SqlxError("completed transfer has no destination snapshot".into())
    })?;
    state
        .db
        .replace_container_item_stacks(work.transfer.from_container, from_contents)
        .await?;
    state
        .db
        .replace_container_item_stacks(work.transfer.to_container, to_contents)
        .await?;
    Ok(())
}

impl WorkUnitLifecycle for TransferWorkUnit {
    async fn claimed(
        &self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> Result<(), MinehouseError> {
        state.db.mark_craft_operation_claimed(id).await?;
        Ok(())
    }

    async fn updated(
        self,
        id: WorkUnitId,
        state: Arc<MinehouseState>,
    ) -> Result<(), MinehouseError> {
        persist_progress(id, self, state).await
    }

    async fn done(self, id: WorkUnitId, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        apply_snapshot(&self, &state).await?;
        persist_progress(id, self, state).await
    }
}

async fn persist_progress(
    id: WorkUnitId,
    work: TransferWorkUnit,
    state: Arc<MinehouseState>,
) -> Result<(), MinehouseError> {
    let work_unit = WorkUnit {
        id,
        data: WorkUnitData::Transfer(work),
    };
    if state
        .db
        .persist_transfer_work_unit_progress(&work_unit)
        .await?
    {
        state.craft_planner_wake.notify();
    }
    Ok(())
}
