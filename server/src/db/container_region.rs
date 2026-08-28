use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use super::{Cube, DatabaseHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "region_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RegionType {
    Bulk,
    OrderPickface,
    UserPickface,
    Putaway,
    Processing,
    Order,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ContainerRegion {
    pub id: Uuid,
    pub r#type: RegionType,
    pub world_region: Cube,
}

impl DatabaseHandle {
    pub async fn list_regions(&self) -> Result<Vec<ContainerRegion>, sqlx::Error> {
        sqlx::query_as::<_, ContainerRegion>(
            "select id, type, world_region from container_region",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_regions_by_type(
        &self,
        region_type: RegionType,
    ) -> Result<Vec<ContainerRegion>, sqlx::Error> {
        sqlx::query_as::<_, ContainerRegion>(
            "select id, type, world_region from container_region where type = $1",
        )
        .bind(region_type)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn insert_region(
        &self,
        region_type: RegionType,
        world_region: Cube,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            "insert into container_region (type, world_region) values ($1, $2) returning id",
        )
        .bind(region_type)
        .bind(world_region)
        .fetch_one(&self.pool)
        .await
    }
}
