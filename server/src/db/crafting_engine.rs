use sqlx::types::Uuid;

use super::{Cube, container_region::RegionType};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CraftingEngine {
    pub id: Uuid,
    pub role: RegionType,
    pub position: Cube,
}
