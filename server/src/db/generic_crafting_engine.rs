use sqlx::types::Uuid;

use super::{Cube, container_region::RegionType};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GenericCraftingEngine {
    pub id: Uuid,
    pub role: RegionType,
    pub in_position: Cube,
    pub out_position: Cube,
}
