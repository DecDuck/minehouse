use common::{vec::Point, work_units::index_region::IndexChunkWorkUnitContainer};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::types::PgCube, types::Uuid};

use super::{Cube, DatabaseHandle};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, oasgen::OaSchema, sqlx::Type,
)]
#[sqlx(type_name = "region_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
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

impl DatabaseHandle {
    pub async fn fetch_all_regions(&self) -> Result<Vec<ContainerRegion>, sqlx::Error> {
        sqlx::query_as!(
            ContainerRegion,
            r#"select
                id,
                type as "type: RegionType",
                world_region as "world_region: Cube"
            from container_region"#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn fetch_region(&self, id: Uuid) -> Result<Option<ContainerRegion>, sqlx::Error> {
        sqlx::query_as!(
            ContainerRegion,
            r#"select
                id,
                type as "type: RegionType",
                world_region as "world_region: Cube"
            from container_region
            where id = $1"#,
            id,
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn create_region(
        &self,
        region_type: RegionType,
        world_region: Cube,
    ) -> Result<ContainerRegion, sqlx::Error> {
        sqlx::query_as!(
            ContainerRegion,
            r#"insert into container_region (type, world_region)
               values ($1, $2)
               returning
                   id,
                   type as "type: RegionType",
                   world_region as "world_region: Cube""#,
            region_type as RegionType,
            cube_to_pg_cube(world_region),
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_region(
        &self,
        id: Uuid,
        region_type: RegionType,
        world_region: Cube,
    ) -> Result<Option<ContainerRegion>, sqlx::Error> {
        sqlx::query_as!(
            ContainerRegion,
            r#"update container_region
               set type = $2, world_region = $3
               where id = $1
               returning
                   id,
                   type as "type: RegionType",
                   world_region as "world_region: Cube""#,
            id,
            region_type as RegionType,
            cube_to_pg_cube(world_region),
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_region(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!("delete from container_region where id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn sync_region_containers(
        &self,
        region_id: Uuid,
        indexed_containers: &[IndexChunkWorkUnitContainer],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let existing_positions = sqlx::query_scalar!(
            r#"select position as "position: Cube"
               from container
               where region_id = $1"#,
            region_id,
        )
        .fetch_all(&mut *tx)
        .await?;

        for indexed_container in indexed_containers {
            let position = point_to_pg_cube(indexed_container.point);
            let capacity = i32::try_from(indexed_container.capacity).map_err(|_| {
                sqlx::Error::Protocol("container capacity exceeds PostgreSQL integer range".into())
            })?;

            sqlx::query!(
                r#"insert into container (region_id, position, capacity)
                   values ($1, $2, $3)
                   on conflict (position) do update
                   set region_id = excluded.region_id,
                       capacity = excluded.capacity"#,
                region_id,
                position,
                capacity,
            )
            .execute(&mut *tx)
            .await?;
        }

        for existing_position in existing_positions {
            let remains_in_region = indexed_containers
                .iter()
                .map(|container| point_to_cube(container.point))
                .any(|position| position == existing_position);
            if remains_in_region {
                continue;
            }

            sqlx::query!(
                "delete from container where region_id = $1 and position = $2",
                region_id,
                cube_to_pg_cube(existing_position),
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await
    }
}

fn point_to_cube(point: Point) -> Cube {
    Cube {
        x1: point.x,
        y1: point.y,
        z1: point.z,
        x2: point.x,
        y2: point.y,
        z2: point.z,
    }
}

fn point_to_pg_cube(point: Point) -> PgCube {
    PgCube::ZeroVolume(vec![point.x, point.y, point.z])
}

fn cube_to_pg_cube(cube: Cube) -> PgCube {
    PgCube::MultiDimension(vec![
        vec![cube.x1, cube.y1, cube.z1],
        vec![cube.x2, cube.y2, cube.z2],
    ])
}
