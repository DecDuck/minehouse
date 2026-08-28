use std::sync::Arc;

use common::rpc::MinehouseError;
use uuid::Uuid;

use crate::{
    planner::{enqueue_transfers, index_region},
    region::{BulkRegion, Region, StockedRegion, plan_move},
    state::MinehouseState,
};

/// Restocks a user pickface region from bulk to satisfy its configured layout.
pub async fn plan(state: &Arc<MinehouseState>, region_id: Uuid) -> Result<(), MinehouseError> {
    let mut pickface = StockedRegion::user_pickface(&state.db, region_id).await?;
    index_region(state, &pickface);

    let mut bulk = BulkRegion::load(&state.db).await?;
    let requests = pickface.demand();
    let transfers = plan_move(&mut bulk, &mut pickface, &requests);
    enqueue_transfers(state, transfers);
    Ok(())
}
