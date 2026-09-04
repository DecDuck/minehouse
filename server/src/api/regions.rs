use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use oasgen::{OaSchema, oasgen};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ApiError;
use crate::{
    db::{
        Cube,
        container_region::{ContainerRegion, RegionType},
    },
    state::MinehouseState,
};

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct RegionPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct RegionWorld {
    pub pos1: RegionPoint,
    pub pos2: RegionPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct Region {
    pub id: String,
    pub r#type: RegionType,
    pub world_region: RegionWorld,
}

#[derive(Debug, Clone, Deserialize, OaSchema)]
pub struct RegionRequest {
    pub r#type: RegionType,
    pub world_region: RegionWorld,
}

impl From<ContainerRegion> for Region {
    fn from(region: ContainerRegion) -> Self {
        Self {
            id: region.id.to_string(),
            r#type: region.r#type,
            world_region: RegionWorld {
                pos1: RegionPoint {
                    x: region.world_region.x1,
                    y: region.world_region.y1,
                    z: region.world_region.z1,
                },
                pos2: RegionPoint {
                    x: region.world_region.x2,
                    y: region.world_region.y2,
                    z: region.world_region.z2,
                },
            },
        }
    }
}

impl From<RegionWorld> for Cube {
    fn from(world_region: RegionWorld) -> Self {
        Self {
            x1: world_region.pos1.x,
            y1: world_region.pos1.y,
            z1: world_region.pos1.z,
            x2: world_region.pos2.x,
            y2: world_region.pos2.y,
            z2: world_region.pos2.z,
        }
    }
}

#[oasgen(summary = "List all container regions")]
pub async fn list_regions(
    State(state): State<Arc<MinehouseState>>,
) -> Result<Json<Vec<Region>>, ApiError> {
    let regions = state.db.fetch_all_regions().await?;
    Ok(Json(regions.into_iter().map(Region::from).collect()))
}

#[oasgen(summary = "Get a container region")]
pub async fn get_region(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<String>,
) -> Result<Json<Region>, ApiError> {
    let id = Uuid::parse_str(&id)?;
    state
        .db
        .fetch_region(id)
        .await?
        .map(|region| Json(region.into()))
        .ok_or_else(|| ApiError::from_debug("region not found"))
}

#[oasgen(summary = "Create a container region")]
pub async fn create_region(
    State(state): State<Arc<MinehouseState>>,
    Json(request): Json<RegionRequest>,
) -> Result<Json<Region>, ApiError> {
    let region = state
        .db
        .create_region(request.r#type, request.world_region.into())
        .await?;
    Ok(Json(region.into()))
}

#[oasgen(summary = "Replace a container region")]
pub async fn update_region(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<String>,
    Json(request): Json<RegionRequest>,
) -> Result<Json<Region>, ApiError> {
    let id = Uuid::parse_str(&id)?;
    state
        .db
        .update_region(id, request.r#type, request.world_region.into())
        .await?
        .map(|region| Json(region.into()))
        .ok_or_else(|| ApiError::from_debug("region not found"))
}

#[oasgen(summary = "Delete a container region")]
pub async fn delete_region(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let id = Uuid::parse_str(&id)?;
    if state.db.delete_region(id).await? {
        Ok(Json(()))
    } else {
        Err(ApiError::from_debug("region not found"))
    }
}
