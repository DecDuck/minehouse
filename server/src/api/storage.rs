use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use oasgen::{OaSchema, oasgen};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use super::ApiError;
use crate::{
    db::storage::{ItemKindStackRow, StorageItemRow},
    state::MinehouseState,
};

#[derive(Debug, Clone, Default, Deserialize, OaSchema)]
pub struct StorageQuery {
    pub q: Option<String>,
    pub region_id: Option<String>,
    pub min_quantity: Option<i64>,
    pub sort_by: Option<StorageSortBy>,
    pub sort_direction: Option<StorageSortDirection>,
}

#[derive(Debug, Clone, Copy, Deserialize, OaSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageSortBy {
    Name,
    Quantity,
    Stacks,
    Containers,
}

impl Default for StorageSortBy {
    fn default() -> Self {
        Self::Name
    }
}

impl StorageSortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Quantity => "quantity",
            Self::Stacks => "stacks",
            Self::Containers => "containers",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, OaSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageSortDirection {
    Asc,
    Desc,
}

impl Default for StorageSortDirection {
    fn default() -> Self {
        Self::Asc
    }
}

impl StorageSortDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct StorageItem {
    pub item_kind: String,
    pub quantity: i64,
    pub stack_count: i64,
    pub container_count: i64,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ItemKindStack {
    pub stack_id: String,
    pub container_id: String,
    pub region_id: String,
    pub region_type: String,
    pub position: ItemPosition,
    pub slot: i32,
    pub quantity: i32,
    pub components: sqlx::types::JsonValue,
    pub components_digest: String,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ItemPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct ItemKindDetails {
    pub item_kind: String,
    pub total_quantity: i64,
    pub stack_count: i64,
    pub container_count: i64,
    pub stacks: Vec<ItemKindStack>,
}

impl From<StorageItemRow> for StorageItem {
    fn from(row: StorageItemRow) -> Self {
        Self {
            item_kind: row.item_kind,
            quantity: row.quantity,
            stack_count: row.stack_count,
            container_count: row.container_count,
        }
    }
}

impl From<ItemKindStackRow> for ItemKindStack {
    fn from(row: ItemKindStackRow) -> Self {
        Self {
            stack_id: row.stack_id.to_string(),
            container_id: row.container_id.to_string(),
            region_id: row.region_id.to_string(),
            region_type: row.region_type,
            position: ItemPosition {
                x: row.position.x1,
                y: row.position.y1,
                z: row.position.z1,
            },
            slot: row.slot,
            quantity: row.quantity,
            components: row.components,
            components_digest: row.components_digest,
        }
    }
}

#[oasgen(summary = "Search indexed storage contents")]
pub async fn list_storage(
    State(state): State<Arc<MinehouseState>>,
    Query(query): Query<StorageQuery>,
) -> Result<Json<Vec<StorageItem>>, ApiError> {
    let region_id = query
        .region_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()?;
    let items = state
        .db
        .search_storage(
            query.q.as_deref(),
            region_id,
            query.min_quantity,
            query.sort_by.unwrap_or_default().as_str(),
            query.sort_direction.unwrap_or_default().as_str(),
        )
        .await?;
    Ok(Json(items.into_iter().map(StorageItem::from).collect()))
}

#[oasgen(summary = "Get all indexed stacks for an item kind")]
pub async fn get_item_kind_details(
    State(state): State<Arc<MinehouseState>>,
    axum::extract::Path(item_kind): axum::extract::Path<String>,
) -> Result<Json<ItemKindDetails>, ApiError> {
    let rows = state.db.fetch_item_kind_stacks(&item_kind).await?;
    let total_quantity = rows.iter().map(|row| i64::from(row.quantity)).sum();
    let container_count = rows
        .iter()
        .map(|row| row.container_id)
        .collect::<HashSet<Uuid>>()
        .len() as i64;
    Ok(Json(ItemKindDetails {
        item_kind,
        total_quantity,
        stack_count: rows.len() as i64,
        container_count,
        stacks: rows.into_iter().map(ItemKindStack::from).collect(),
    }))
}
