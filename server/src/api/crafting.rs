use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use oasgen::{OaSchema, oasgen};
use serde::Serialize;

use crate::{
    api::ApiError,
    mwms::crafting::{CraftJobStatus, CraftPlan, CraftPlanRequest, resolve_plan},
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
            output_item_kind: recipe.output_item_kind,
            output_yield: recipe.output_yield,
            engine_type: recipe.engine_type,
            ingredients: recipe
                .ingredients
                .into_iter()
                .map(|ingredient| IngredientResponse {
                    item_kind: ingredient.item_kind,
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
) -> Result<Json<CraftJobStatus>, ApiError> {
    let id = uuid::Uuid::parse_str(&id)?;
    state
        .db
        .fetch_craft_job(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::from_debug(format!("craft job {id} was not found")))
}
