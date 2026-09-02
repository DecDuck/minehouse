use std::sync::Arc;

use axum::{Json, extract::State};
use oasgen::oasgen;

use crate::{mwms::planner::PlannerRequest, state::MinehouseState};

#[oasgen(summary = "Queue a planner request")]
pub async fn queue_request(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<PlannerRequest>,
) -> Json<()> {
    state.planner_queue.push(request);
    Json(())
}
