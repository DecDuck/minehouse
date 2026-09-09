use sqlx::types::Uuid;

use super::Cube;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "crafting_engine_type", rename_all = "snake_case")]
pub enum CraftingEngineType {
    CraftingTable,
    Smithing,
    Stonecutter,
}

impl CraftingEngineType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CraftingTable => "crafting_table",
            Self::Smithing => "smithing",
            Self::Stonecutter => "stonecutter",
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CraftingRegionAssignment {
    pub engine_id: Uuid,
    pub region_id: Uuid,
    pub engine_position: Cube,
    pub container_id: Uuid,
    pub container_position: Cube,
    pub container_capacity: i32,
    pub inherited_from: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct InheritedCraftingRegionAssignment {
    source_node_id: Uuid,
    engine_id: Uuid,
    region_id: Uuid,
    engine_position: Cube,
    container_id: Uuid,
    container_position: Cube,
    container_capacity: i32,
}

impl super::DatabaseHandle {
    pub async fn try_lease_crafting_region(
        &self,
        node_id: Uuid,
        engine_type: CraftingEngineType,
    ) -> Result<Option<CraftingRegionAssignment>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        if let Some(assignment) = sqlx::query_as!(
            CraftingRegionAssignment,
            r#"select
                   engine.id as "engine_id!",
                   engine.region_id as "region_id!",
                   engine.position as "engine_position: Cube",
                   container.id as "container_id!",
                   container.position as "container_position: Cube",
                   container.capacity as "container_capacity!",
                   null::uuid as inherited_from
               from craft_processing_lease as lease
               join crafting_engine as engine on engine.region_id = lease.region_id
               join lateral (
                   select id, position, capacity
                   from container
                   where region_id = engine.region_id
                   order by id
                   limit 1
               ) as container on true
                             where lease.node_id = $1
                                 and lease.indexed_after_startup"#,
            node_id,
        )
        .fetch_optional(&mut *transaction)
        .await?
        {
            transaction.commit().await?;
            return Ok(Some(assignment));
        }

        if let Some(inherited) = sqlx::query_as!(
            InheritedCraftingRegionAssignment,
            r#"select
                   child.id as "source_node_id!",
                   engine.id as "engine_id!",
                   engine.region_id as "region_id!",
                   engine.position as "engine_position: Cube",
                   container.id as "container_id!",
                   container.position as "container_position: Cube",
                   container.capacity as "container_capacity!"
               from craft_job_node as child
               join craft_processing_lease as lease on lease.node_id = child.id
               join crafting_engine as engine on engine.region_id = lease.region_id
               join lateral (
                   select id, position, capacity
                   from container
                   where region_id = engine.region_id
                   order by id
                   limit 1
               ) as container on true
                             where child.parent_id = $1
                 and child.state = 'completed'
                 and engine.engine_type = $2
                                 and lease.indexed_after_startup
               order by child.child_order
               limit 1
               for update of lease skip locked"#,
            node_id,
            engine_type as CraftingEngineType,
        )
        .fetch_optional(&mut *transaction)
        .await?
        {
            sqlx::query!(
                "update craft_processing_lease set node_id = $2, leased_at = now() where node_id = $1",
                inherited.source_node_id,
                node_id,
            )
            .execute(&mut *transaction)
            .await?;
            sqlx::query!(
                "update craft_job_node set assigned_region_id = null, updated_at = now() where id = $1",
                inherited.source_node_id,
            )
            .execute(&mut *transaction)
            .await?;
            sqlx::query!(
                "update craft_job_node set assigned_region_id = $2, state = 'staging', updated_at = now() where id = $1",
                node_id,
                inherited.region_id,
            )
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            return Ok(Some(CraftingRegionAssignment {
                engine_id: inherited.engine_id,
                region_id: inherited.region_id,
                engine_position: inherited.engine_position,
                container_id: inherited.container_id,
                container_position: inherited.container_position,
                container_capacity: inherited.container_capacity,
                inherited_from: Some(inherited.source_node_id),
            }));
        }

        let assignment = sqlx::query_as!(
            CraftingRegionAssignment,
            r#"select
                   engine.id as "engine_id!",
                   engine.region_id as "region_id!",
                   engine.position as "engine_position: Cube",
                   container.id as "container_id!",
                   container.position as "container_position: Cube",
                   container.capacity as "container_capacity!",
                   null::uuid as inherited_from
               from crafting_engine as engine
               join container_region as region
                 on region.id = engine.region_id
                and region.type = 'processing'
               join lateral (
                   select id, position, capacity
                   from container
                   where region_id = engine.region_id
                   order by id
                   limit 1
               ) as container on true
               where engine.engine_type = $1
                 and not exists (
                     select 1
                     from craft_processing_lease as lease
                     where lease.region_id = engine.region_id
                 )
               order by region.priority desc, engine.id
               limit 1
               for update of engine skip locked"#,
            engine_type as CraftingEngineType,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(assignment) = assignment else {
            transaction.rollback().await?;
            return Ok(None);
        };

        sqlx::query!(
            "insert into craft_processing_lease (region_id, node_id, indexed_after_startup) values ($1, $2, true)",
            assignment.region_id,
            node_id,
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query!(
            "update craft_job_node set assigned_region_id = $2, state = 'staging', updated_at = now() where id = $1",
            node_id,
            assignment.region_id,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(assignment))
    }

    pub async fn release_crafting_region(
        &self,
        node_id: Uuid,
        restore_to: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        if let Some(source_node_id) = restore_to {
            let region_id = sqlx::query_scalar!(
                "update craft_processing_lease set node_id = $2, leased_at = now() where node_id = $1 returning region_id",
                node_id,
                source_node_id,
            )
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query!(
                "update craft_job_node set assigned_region_id = $2, updated_at = now() where id = $1",
                source_node_id,
                region_id,
            )
            .execute(&mut *transaction)
            .await?;
        } else {
            sqlx::query!(
                "delete from craft_processing_lease where node_id = $1",
                node_id,
            )
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query!(
            "update craft_job_node set assigned_region_id = null, state = 'blocked', updated_at = now() where id = $1",
            node_id,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }
}
