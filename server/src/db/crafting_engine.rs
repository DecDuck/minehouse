use sqlx::types::Uuid;

use super::{Cube, container_region::RegionType};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CraftingEngine {
    pub id: Uuid,
    pub role: RegionType,
    pub position: Cube,
}

impl super::DatabaseHandle {
    pub async fn fetch_crafting_engine(
        &self,
        role: RegionType,
    ) -> Result<Option<CraftingEngine>, sqlx::Error> {
        sqlx::query_as!(
            CraftingEngine,
            "select id, role as \"role: RegionType\", position as \"position: Cube\" from crafting_engine where role = $1 order by id limit 1",
            role as RegionType,
        )
        .fetch_optional(&self.pool)
        .await
    }
}
