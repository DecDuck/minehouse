use std::sync::Arc;

use axum::{Json, extract::State};
use oasgen::{OaSchema, oasgen};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    api::ApiError,
    mwms::{
        crafting::{CraftJobState, CraftJobStatus},
        planner::PlannerRequest,
    },
    state::MinehouseState,
};

#[derive(Debug, Serialize, OaSchema)]
pub struct QueueResponse {
    pub job_id: Option<String>,
}

#[oasgen(summary = "Queue a planner request")]
pub async fn queue_request(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<PlannerRequest>,
) -> Result<Json<QueueResponse>, ApiError> {
    let job_id = match request {
        PlannerRequest::Craft(mut request) => {
            let job_id = Uuid::new_v4();
            request.job_id = Some(job_id.to_string());
            state
                .db
                .create_craft_job(&CraftJobStatus {
                    id: job_id.to_string(),
                    target_item_kind: request.item_kind.clone(),
                    target_quantity: request.amount,
                    selections: request.selections.clone(),
                    state: CraftJobState::Waiting,
                    completed_quantity: 0,
                    error: None,
                })
                .await?;
            state.planner_queue.push(PlannerRequest::Craft(request));
            Some(job_id.to_string())
        }
        request => {
            state.planner_queue.push(request);
            None
        }
    };
    Ok(Json(QueueResponse { job_id }))
}
