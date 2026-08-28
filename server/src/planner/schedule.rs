use std::{sync::Arc, time::Duration};

use tokio::time::interval;

use crate::{planner::PlannerOperation, state::MinehouseState};

/// Periodically enqueues a full reconcile so the warehouse converges without an
/// external trigger.
pub async fn run(state: Arc<MinehouseState>, period: Duration) {
    let mut ticker = interval(period);
    loop {
        ticker.tick().await;
        state.planner.enqueue(PlannerOperation::ReconcileAll).await;
    }
}
