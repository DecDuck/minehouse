use sqlx::types::Uuid;

use super::Cube;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Container {
    pub id: Uuid,
    pub region_id: Uuid,
    pub position: Cube,
    pub capacity: i32,
}
