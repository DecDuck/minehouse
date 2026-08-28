use std::sync::Arc;

use common::rpc::MinehouseError;

use crate::{
    db::container_region::RegionType,
    planner::{
        PlannerOperation, container_point, enqueue_index, order_pickface, putaway, restock,
    },
    state::MinehouseState,
};

/// Consumes planner operations forever, turning each into work units.
pub async fn run(state: Arc<MinehouseState>) {
    loop {
        let task = state.planner.next().await;
        if let Err(err) = handle(&state, &task.operation).await {
            tracing::error!("planner operation {:?} failed: {:?}", task.operation, err);
        }
        state.planner.finish_current().await;
    }
}

async fn handle(
    state: &Arc<MinehouseState>,
    operation: &PlannerOperation,
) -> Result<(), MinehouseError> {
    match operation {
        PlannerOperation::ReconcileAll => reconcile_all(state).await,
        PlannerOperation::Putaway { region_id } => putaway::plan(state, *region_id).await,
        PlannerOperation::RestockUserPickface { region_id } => {
            restock::plan(state, *region_id).await
        }
        PlannerOperation::SyncOrderPickface { region_id } => {
            order_pickface::plan(state, *region_id).await
        }
        PlannerOperation::IndexContainer { container_id } => {
            if let Some(container) = state.db.get_container(*container_id).await? {
                enqueue_index(state, container.id, container_point(&container));
            }
            Ok(())
        }
    }
}

/// Enqueues a reconcile operation for every putaway, user pickface and order
/// pickface region.
async fn reconcile_all(state: &Arc<MinehouseState>) -> Result<(), MinehouseError> {
    for region in state.db.list_regions_by_type(RegionType::Putaway).await? {
        state
            .planner
            .enqueue(PlannerOperation::Putaway {
                region_id: region.id,
            })
            .await;
    }
    for region in state.db.list_regions_by_type(RegionType::UserPickface).await? {
        state
            .planner
            .enqueue(PlannerOperation::RestockUserPickface {
                region_id: region.id,
            })
            .await;
    }
    for region in state.db.list_regions_by_type(RegionType::OrderPickface).await? {
        state
            .planner
            .enqueue(PlannerOperation::SyncOrderPickface {
                region_id: region.id,
            })
            .await;
    }
    Ok(())
}
