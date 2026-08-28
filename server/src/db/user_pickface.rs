use serde::Serialize;
use sqlx::types::Uuid;

use super::DatabaseHandle;

/// A user-defined pickface: per-slot desired `item_kind` (empty string = leave alone).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserPickface {
    pub container_id: Uuid,
    pub item_slots: Vec<String>,
}

impl DatabaseHandle {
    pub async fn list_user_pickfaces(&self) -> Result<Vec<UserPickface>, sqlx::Error> {
        sqlx::query_as::<_, UserPickface>("select container_id, item_slots from user_pickface")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn get_user_pickface(
        &self,
        container_id: Uuid,
    ) -> Result<Option<UserPickface>, sqlx::Error> {
        sqlx::query_as::<_, UserPickface>(
            "select container_id, item_slots from user_pickface where container_id = $1",
        )
        .bind(container_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn upsert_user_pickface(
        &self,
        container_id: Uuid,
        item_slots: &[String],
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "insert into user_pickface (container_id, item_slots) values ($1, $2)
             on conflict (container_id) do update set item_slots = excluded.item_slots",
        )
        .bind(container_id)
        .bind(item_slots)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
