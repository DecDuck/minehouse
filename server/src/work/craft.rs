use std::sync::Arc;

use common::{rpc::MinehouseError, work_units::craft::CraftWorkUnit};

use super::WorkUnitAction;
use crate::state::MinehouseState;

impl WorkUnitAction for CraftWorkUnit {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        let input = self.progress.input_snapshot.ok_or_else(|| {
            MinehouseError::SqlxError("completed craft has no input snapshot".into())
        })?;
        let output = self.progress.output_snapshot.ok_or_else(|| {
            MinehouseError::SqlxError("completed craft has no output snapshot".into())
        })?;
        if self.locations.input_container_id == self.locations.output_container_id {
            state
                .db
                .replace_container_item_stacks(self.locations.output_container_id, &output)
                .await?;
        } else {
            state
                .db
                .replace_container_item_stacks(self.locations.input_container_id, &input)
                .await?;
            state
                .db
                .replace_container_item_stacks(self.locations.output_container_id, &output)
                .await?;
        }
        Ok(())
    }
}
