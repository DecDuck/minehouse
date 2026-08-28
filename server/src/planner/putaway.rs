use std::sync::Arc;

use common::rpc::MinehouseError;
use uuid::Uuid;

use crate::{
    planner::{enqueue_transfers, index_region},
    region::{BulkRegion, PutawayRegion, Region, plan_move},
    state::MinehouseState,
};

/// Drains a putaway region into bulk, letting bulk place items by category.
pub async fn plan(state: &Arc<MinehouseState>, region_id: Uuid) -> Result<(), MinehouseError> {
    let mut putaway = PutawayRegion::load(&state.db, region_id).await?;
    index_region(state, &putaway);

    let mut bulk = BulkRegion::load(&state.db).await?;
    let requests = putaway.surplus();
    let transfers = plan_move(&mut putaway, &mut bulk, &requests);
    enqueue_transfers(state, transfers);
    Ok(())
}
