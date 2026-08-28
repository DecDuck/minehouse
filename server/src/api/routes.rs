use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use common::{
    item_stack::ItemStack,
    vec::{Point, PointRegion},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{
        Cube,
        container::Container,
        container_region::{ContainerRegion, RegionType},
        user_pickface::UserPickface,
    },
    mwms::category::ItemCategory,
    planner::{PlannerOperation, PlannerQueueSnapshot, PlannerTaskId},
    state::MinehouseState,
};

pub fn router() -> Router<Arc<MinehouseState>> {
    Router::new()
        .route("/api/planner/reconcile", post(reconcile))
        .route("/api/planner/putaway/{region_id}", post(putaway))
        .route("/api/planner/restock/{region_id}", post(restock))
        .route(
            "/api/planner/order-pickface/{region_id}",
            post(order_pickface),
        )
        .route("/api/planner/index/{container_id}", post(index_container))
        .route("/api/planner/queue", get(queue_snapshot))
        .route("/api/regions", get(list_regions).post(create_region))
        .route("/api/containers", get(list_containers).post(create_container))
        .route("/api/containers/{id}/category", put(set_container_category))
        .route("/api/containers/{id}/contents", get(container_contents))
        .route("/api/user-pickfaces", get(list_user_pickfaces))
        .route(
            "/api/user-pickfaces/{container_id}",
            put(set_user_pickface),
        )
}

#[derive(Serialize)]
struct EnqueueResponse {
    task_id: Option<PlannerTaskId>,
}

#[derive(Serialize)]
struct IdResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct CreateRegionRequest {
    region_type: RegionType,
    world_region: PointRegion,
}

#[derive(Deserialize)]
struct CreateContainerRequest {
    region_id: Uuid,
    position: Point,
    capacity: i32,
    #[serde(default)]
    category: Option<ItemCategory>,
}

#[derive(Deserialize)]
struct SetCategoryRequest {
    #[serde(default)]
    category: Option<ItemCategory>,
}

#[derive(Deserialize)]
struct SetUserPickfaceRequest {
    item_slots: Vec<String>,
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:?}"))
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

async fn reconcile(State(state): State<Arc<MinehouseState>>) -> Json<EnqueueResponse> {
    let task_id = state.planner.enqueue(PlannerOperation::ReconcileAll).await;
    Json(EnqueueResponse { task_id })
}

async fn putaway(
    State(state): State<Arc<MinehouseState>>,
    Path(region_id): Path<Uuid>,
) -> Json<EnqueueResponse> {
    let task_id = state
        .planner
        .enqueue(PlannerOperation::Putaway { region_id })
        .await;
    Json(EnqueueResponse { task_id })
}

async fn restock(
    State(state): State<Arc<MinehouseState>>,
    Path(region_id): Path<Uuid>,
) -> Json<EnqueueResponse> {
    let task_id = state
        .planner
        .enqueue(PlannerOperation::RestockUserPickface { region_id })
        .await;
    Json(EnqueueResponse { task_id })
}

async fn order_pickface(
    State(state): State<Arc<MinehouseState>>,
    Path(region_id): Path<Uuid>,
) -> Json<EnqueueResponse> {
    let task_id = state
        .planner
        .enqueue(PlannerOperation::SyncOrderPickface { region_id })
        .await;
    Json(EnqueueResponse { task_id })
}

async fn index_container(
    State(state): State<Arc<MinehouseState>>,
    Path(container_id): Path<Uuid>,
) -> Json<EnqueueResponse> {
    let task_id = state
        .planner
        .enqueue(PlannerOperation::IndexContainer { container_id })
        .await;
    Json(EnqueueResponse { task_id })
}

async fn queue_snapshot(State(state): State<Arc<MinehouseState>>) -> Json<PlannerQueueSnapshot> {
    Json(state.planner.snapshot().await)
}

async fn list_regions(State(state): State<Arc<MinehouseState>>) -> ApiResult<Vec<ContainerRegion>> {
    Ok(Json(state.db.list_regions().await?))
}

async fn create_region(
    State(state): State<Arc<MinehouseState>>,
    Json(req): Json<CreateRegionRequest>,
) -> ApiResult<IdResponse> {
    let id = state
        .db
        .insert_region(req.region_type, req.world_region.into())
        .await?;
    Ok(Json(IdResponse { id }))
}

async fn list_containers(State(state): State<Arc<MinehouseState>>) -> ApiResult<Vec<Container>> {
    Ok(Json(state.db.list_containers().await?))
}

async fn create_container(
    State(state): State<Arc<MinehouseState>>,
    Json(req): Json<CreateContainerRequest>,
) -> ApiResult<IdResponse> {
    // A container occupies a single block, stored as a degenerate point-cube.
    let position: Cube = PointRegion {
        pos1: req.position,
        pos2: req.position,
    }
    .into();
    let id = state
        .db
        .insert_container(req.region_id, position, req.capacity, req.category)
        .await?;
    Ok(Json(IdResponse { id }))
}

async fn set_container_category(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetCategoryRequest>,
) -> Result<StatusCode, ApiError> {
    state.db.set_container_category(id, req.category).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn container_contents(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Vec<ItemStack>> {
    Ok(Json(state.db.get_container_contents(id).await?))
}

async fn list_user_pickfaces(
    State(state): State<Arc<MinehouseState>>,
) -> ApiResult<Vec<UserPickface>> {
    Ok(Json(state.db.list_user_pickfaces().await?))
}

async fn set_user_pickface(
    State(state): State<Arc<MinehouseState>>,
    Path(container_id): Path<Uuid>,
    Json(req): Json<SetUserPickfaceRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .upsert_user_pickface(container_id, &req.item_slots)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
