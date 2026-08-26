use std::sync::Arc;

use common::{rpc::MinehouseError, work_units::index_container::IndexContainerWorkUnit};

use crate::{state::MinehouseState, work::WorkUnitAction};

impl WorkUnitAction for IndexContainerWorkUnit {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        state
            .db
            .replace_container_item_stacks(self.container_id, &self.output.unwrap())
            .await?;
        Ok(())
    }
}
