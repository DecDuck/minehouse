use std::sync::Arc;

use common::{rpc::MinehouseError, work_units::transfer::TransferWorkUnit};

use crate::{state::MinehouseState, work::WorkUnitAction};

impl WorkUnitAction for TransferWorkUnit {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        // The worker captured both containers' post-transfer contents in-situ, so
        // persist them directly rather than re-indexing (which would need travel).
        if let Some(from_contents) = &self.from_contents {
            state
                .db
                .replace_container_item_stacks(self.transfer.from_container, from_contents)
                .await?;
        }
        if let Some(to_contents) = &self.to_contents {
            state
                .db
                .replace_container_item_stacks(self.transfer.to_container, to_contents)
                .await?;
        }
        Ok(())
    }
}
