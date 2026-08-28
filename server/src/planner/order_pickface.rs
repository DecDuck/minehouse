use std::sync::Arc;

use common::rpc::MinehouseError;
use uuid::Uuid;

use crate::{
    planner::{enqueue_transfers, index_region},
    region::{BulkRegion, Region, StockedRegion, plan_move},
    state::MinehouseState,
};

/// Keeps an order pickface region stocked from bulk with a representative slice.
pub async fn plan(state: &Arc<MinehouseState>, region_id: Uuid) -> Result<(), MinehouseError> {
    let mut order = StockedRegion::order_pickface(&state.db, region_id).await?;
    index_region(state, &order);

    let mut bulk = BulkRegion::load(&state.db).await?;
    let requests = order.demand();
    let transfers = plan_move(&mut bulk, &mut order, &requests);
    enqueue_transfers(state, transfers);
    Ok(())
}
