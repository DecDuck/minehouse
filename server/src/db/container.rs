use serde::Serialize;
use sqlx::types::Uuid;

use super::{Cube, DatabaseHandle, container_region::RegionType};
use crate::mwms::category::ItemCategory;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Container {
    pub id: Uuid,
    pub region_id: Uuid,
    pub position: Cube,
    pub capacity: i32,
    /// Category this container is pinned to (bulk / order-pickface); `None` otherwise.
    pub category: Option<ItemCategory>,
}

impl DatabaseHandle {
    pub async fn get_container(&self, id: Uuid) -> Result<Option<Container>, sqlx::Error> {
        sqlx::query_as::<_, Container>(
            "select id, region_id, position, capacity, category from container where id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_containers(&self) -> Result<Vec<Container>, sqlx::Error> {
        sqlx::query_as::<_, Container>(
            "select id, region_id, position, capacity, category from container",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_containers_in_region(
        &self,
        region_id: Uuid,
    ) -> Result<Vec<Container>, sqlx::Error> {
        sqlx::query_as::<_, Container>(
            "select id, region_id, position, capacity, category from container where region_id = $1",
        )
        .bind(region_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_containers_by_region_type(
        &self,
        region_type: RegionType,
    ) -> Result<Vec<Container>, sqlx::Error> {
        sqlx::query_as::<_, Container>(
            "select c.id, c.region_id, c.position, c.capacity, c.category
             from container c
             join container_region r on r.id = c.region_id
             where r.type = $1",
        )
        .bind(region_type)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_bulk_containers_by_category(
        &self,
        category: ItemCategory,
    ) -> Result<Vec<Container>, sqlx::Error> {
        sqlx::query_as::<_, Container>(
            "select c.id, c.region_id, c.position, c.capacity, c.category
             from container c
             join container_region r on r.id = c.region_id
             where r.type = 'bulk' and c.category = $1",
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn insert_container(
        &self,
        region_id: Uuid,
        position: Cube,
        capacity: i32,
        category: Option<ItemCategory>,
    ) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar::<_, Uuid>(
            "insert into container (region_id, position, capacity, category)
             values ($1, $2, $3, $4) returning id",
        )
        .bind(region_id)
        .bind(position)
        .bind(capacity)
        .bind(category)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn set_container_category(
        &self,
        id: Uuid,
        category: Option<ItemCategory>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("update container set category = $2 where id = $1")
            .bind(id)
            .bind(category)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
