use std::sync::Arc;

use crate::{api::ApiError, mwms::planner::PlannerRequest, state::MinehouseState};
use axum::{Json, extract::State};
use oasgen::{OaSchema, oasgen};
use serde::Serialize;

#[derive(Debug, Serialize, OaSchema)]
pub struct QueueResponse {
    pub job_id: Option<String>,
}

#[oasgen(summary = "Queue a planner request")]
pub async fn queue_request(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<PlannerRequest>,
) -> Result<Json<QueueResponse>, ApiError> {
    state.planner_queue.push(request);
    Ok(Json(QueueResponse { job_id: None }))
}
