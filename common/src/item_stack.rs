use serde::{Deserialize, Serialize};
use sqlx::types::{JsonValue, Uuid};

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: Uuid,
    pub container_id: Uuid,
    pub item_kind: String,
    pub slot: i32,
    pub components: JsonValue,
    pub quantity: i32,
    pub components_digest: String,
}
