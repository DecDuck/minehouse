use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use oasgen::{OaSchema, oasgen};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    api::ApiError,
    mwms::crafting::{
        CraftJobDetail, CraftJobState, CraftJobStatus, CraftPlan, CraftPlanRequest,
        execution_nodes, execution_tree, resolve_plan,
    },
    state::MinehouseState,
};

#[derive(Debug, Serialize, OaSchema)]
pub struct RecipeResponse {
    pub id: String,
    pub output_item_kind: String,
    pub output_yield: u32,
    pub engine_type: String,
    pub ingredients: Vec<IngredientResponse>,
}

#[derive(Debug, Serialize, OaSchema)]
pub struct IngredientResponse {
    pub item_kind: String,
    pub quantity: u32,
}

#[derive(Debug, Serialize, OaSchema)]
pub struct CraftQueueResponse {
    pub job_id: String,
}

#[oasgen(summary = "List available crafting recipes")]
pub async fn list_recipes(
    State(state): State<Arc<MinehouseState>>,
) -> Result<Json<Vec<RecipeResponse>>, ApiError> {
    let recipes = state
        .db
        .fetch_all_recipes()
        .await?
        .into_iter()
        .map(|recipe| RecipeResponse {
            id: recipe.id.to_string(),
            output_item_kind: recipe.output_item_kind.to_string(),
            output_yield: recipe.output_yield,
            engine_type: recipe.engine_type.as_str().to_owned(),
            ingredients: recipe
                .ingredients
                .into_iter()
                .map(|ingredient| IngredientResponse {
                    item_kind: ingredient.item_kind.to_string(),
                    quantity: ingredient.quantity,
                })
                .collect(),
        })
        .collect();
    Ok(Json(recipes))
}

#[oasgen(summary = "Resolve a crafting plan")]
pub async fn resolve_crafting_plan(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<CraftPlanRequest>,
) -> Result<Json<CraftPlan>, ApiError> {
    Ok(Json(resolve_plan(&state.db, &request).await?))
}

#[oasgen(summary = "Queue a resolved craft plan")]
pub async fn queue_craft(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<CraftPlanRequest>,
) -> Result<Json<CraftQueueResponse>, ApiError> {
    let plan = resolve_plan(&state.db, &request).await?;
    if plan.unresolved {
        return Err(ApiError::from_debug(
            "craft plan contains unresolved choices",
        ));
    }
    let job_id = Uuid::new_v4();
    let nodes = execution_nodes(&state.db, &plan.root).await?;
    state
        .db
        .create_craft_job(
            &CraftJobStatus {
                id: job_id.to_string(),
                target_item_kind: request.item_kind.clone(),
                target_quantity: request.amount,
                selections: request.selections,
                state: CraftJobState::Waiting,
                completed_quantity: 0,
                error: None,
            },
            &nodes,
        )
        .await?;
    state.craft_planner_wake.notify();
    Ok(Json(CraftQueueResponse {
        job_id: job_id.to_string(),
    }))
}

#[oasgen(summary = "List craft job statuses")]
pub async fn list_craft_statuses(
    State(state): State<Arc<MinehouseState>>,
) -> Result<Json<Vec<CraftJobStatus>>, ApiError> {
    Ok(Json(state.db.fetch_craft_jobs().await?))
}

#[oasgen(summary = "Get a craft job status")]
pub async fn get_craft_status(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<String>,
) -> Result<Json<CraftJobDetail>, ApiError> {
    let id = uuid::Uuid::parse_str(&id)?;
    let job = state
        .db
        .fetch_craft_job(id)
        .await?
        .ok_or_else(|| ApiError::from_debug(format!("craft job {id} was not found")))?;
    let root = execution_tree(state.db.fetch_craft_job_nodes(id).await?)?;
    Ok(Json(CraftJobDetail { job, root }))
}
