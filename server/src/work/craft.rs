use std::sync::Arc;

use common::{
    ids::WorkUnitId,
    rpc::MinehouseError,
    work::{WorkUnit, WorkUnitData},
    work_units::craft::CraftWorkUnit,
};

use super::WorkUnitLifecycle;
use crate::state::MinehouseState;

async fn apply_snapshot(
    work: &CraftWorkUnit,
    state: &MinehouseState,
) -> Result<(), MinehouseError> {
    let input =
        work.progress.input_snapshot.as_ref().ok_or_else(|| {
            MinehouseError::SqlxError("completed craft has no input snapshot".into())
        })?;
    let output = work.progress.output_snapshot.as_ref().ok_or_else(|| {
        MinehouseError::SqlxError("completed craft has no output snapshot".into())
    })?;
    if work.locations.input_container_id == work.locations.output_container_id {
        state
            .db
            .replace_container_item_stacks(work.locations.output_container_id, output)
            .await?;
    } else {
        state
            .db
            .replace_container_item_stacks(work.locations.input_container_id, input)
            .await?;
        state
            .db
            .replace_container_item_stacks(work.locations.output_container_id, output)
            .await?;
    }
    Ok(())
}

impl WorkUnitLifecycle for CraftWorkUnit {
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
        if self.progress.input_snapshot.is_some() && self.progress.output_snapshot.is_some() {
            apply_snapshot(&self, &state).await?;
        }
        persist_progress(id, self, state).await
    }

    async fn done(self, id: WorkUnitId, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        apply_snapshot(&self, &state).await?;
        persist_progress(id, self, state).await
    }
}

async fn persist_progress(
    id: WorkUnitId,
    work: CraftWorkUnit,
    state: Arc<MinehouseState>,
) -> Result<(), MinehouseError> {
    let work_unit = WorkUnit {
        id,
        data: WorkUnitData::Craft(work),
    };
    if state
        .db
        .persist_craft_work_unit_progress(&work_unit)
        .await?
    {
        state.craft_planner_wake.notify();
    }
    Ok(())
}
