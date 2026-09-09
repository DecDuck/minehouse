use crate::mwms::storage::StorageEndpointError;
use axum::{Json, http::StatusCode, response::IntoResponse};
use common::rpc::MinehouseError;

pub struct ApiError(String);

impl ApiError {
    pub fn from_debug<E: std::fmt::Debug>(error: E) -> Self {
        Self(format!("{error:?}"))
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::from_debug(error)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::from_debug(error)
    }
}

impl From<uuid::Error> for ApiError {
    fn from(error: uuid::Error) -> Self {
        Self::from_debug(error)
    }
}

impl From<MinehouseError> for ApiError {
    fn from(error: MinehouseError) -> Self {
        Self::from_debug(error)
    }
}

impl From<StorageEndpointError> for ApiError {
    fn from(error: StorageEndpointError) -> Self {
        Self::from_debug(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self.0)).into_response()
    }
}

pub mod containers;
pub mod crafting;
pub mod queue;
pub mod regions;
pub mod status;
pub mod storage;

pub use containers::{get_container, list_containers, list_containers_grouped};
pub use crafting::{
    get_craft_status, list_craft_statuses, list_recipes, queue_craft, resolve_crafting_plan,
};
pub use queue::queue_request;
pub use regions::{create_region, delete_region, get_region, list_regions, update_region};
pub use status::get_system_status;
pub use storage::{get_item_kind_details, list_storage};
