use sqlx::types::Uuid;

use super::Cube;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "region_type", rename_all = "snake_case")]
pub enum RegionType {
    Bulk,
    Pickface,
    Putaway,
    Processing,
    Order,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ContainerRegion {
    pub id: Uuid,
    pub r#type: RegionType,
    pub world_region: Cube,
}
