use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use common::item_stack::ItemStack;
use oasgen::{OaSchema, oasgen};
use serde::Serialize;
use sqlx::types::JsonValue;
use uuid::Uuid;

use super::ApiError;
use crate::{
    db::{container::Container, container_region::RegionType},
    state::MinehouseState,
};

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ContainerPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ItemStackView {
    pub id: String,
    pub container_id: String,
    pub item_kind: String,
    pub slot: i32,
    pub components: JsonValue,
    pub quantity: i32,
    pub components_digest: String,
}

impl From<ItemStack> for ItemStackView {
    fn from(item: ItemStack) -> Self {
        Self {
            id: item.id.to_string(),
            container_id: item.container_id.to_string(),
            item_kind: item.item_kind,
            slot: item.slot,
            components: item.components,
            quantity: item.quantity,
            components_digest: item.components_digest,
        }
    }
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ContainerView {
    pub id: String,
    pub region_id: String,
    pub position: ContainerPosition,
    pub capacity: u64,
    pub contents: Vec<ItemStackView>,
}

impl From<Container> for ContainerView {
    fn from(container: Container) -> Self {
        Self {
            id: container.id.to_string(),
            region_id: container.region_id.to_string(),
            position: ContainerPosition {
                x: container.position.x1,
                y: container.position.y1,
                z: container.position.z1,
            },
            capacity: container.capacity,
            contents: container
                .contents
                .into_values()
                .map(ItemStackView::from)
                .collect(),
        }
    }
}

#[oasgen(summary = "List all containers with their current contents")]
pub async fn list_containers(
    State(state): State<Arc<MinehouseState>>,
) -> Result<Json<Vec<ContainerView>>, ApiError> {
    Ok(Json(
        state
            .db
            .fetch_all_containers()
            .await?
            .into_iter()
            .map(ContainerView::from)
            .collect(),
    ))
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ContainerRegionGroup {
    pub region_id: String,
    pub region_type: Option<RegionType>,
    pub priority: Option<i32>,
    pub containers: Vec<ContainerView>,
}

#[oasgen(summary = "List all containers grouped by region")]
pub async fn list_containers_grouped(
    State(state): State<Arc<MinehouseState>>,
) -> Result<Json<Vec<ContainerRegionGroup>>, ApiError> {
    let regions = state.db.fetch_all_regions().await?;
    let region_index: HashMap<Uuid, (RegionType, i32)> = regions
        .into_iter()
        .map(|region| (region.id, (region.r#type, region.priority)))
        .collect();

    let mut groups: HashMap<Uuid, Vec<ContainerView>> = HashMap::new();
    for container in state.db.fetch_all_containers().await? {
        groups
            .entry(container.region_id)
            .or_default()
            .push(ContainerView::from(container));
    }

    let mut result: Vec<ContainerRegionGroup> = groups
        .into_iter()
        .map(|(region_id, containers)| {
            let info = region_index.get(&region_id);
            ContainerRegionGroup {
                region_id: region_id.to_string(),
                region_type: info.map(|(region_type, _)| *region_type),
                priority: info.map(|(_, priority)| *priority),
                containers,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.region_id.cmp(&b.region_id))
    });

    Ok(Json(result))
}

#[oasgen(summary = "Get a container with its current contents")]
pub async fn get_container(
    State(state): State<Arc<MinehouseState>>,
    Path(id): Path<String>,
) -> Result<Json<ContainerView>, ApiError> {
    let id = Uuid::parse_str(&id)?;
    state
        .db
        .fetch_container(id)
        .await?
        .map(|container| Json(container.into()))
        .ok_or_else(|| ApiError::from_debug("container not found"))
}
