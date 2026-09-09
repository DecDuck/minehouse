use common::{
    ids::WorkUnitId,
    work::{WorkUnit, WorkUnitData, WorkUnitDone as _},
    work_units::craft::CraftIngredient,
};
use uuid::Uuid;

use super::crafting_engine::CraftingEngineType;
use crate::mwms::crafting::{CraftJobState, CraftJobStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "craft_node_kind", rename_all = "snake_case")]
pub enum CraftNodeKind {
    Recipe,
    Storage,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "craft_node_state", rename_all = "snake_case")]
pub enum CraftNodeState {
    Pending,
    Blocked,
    Staging,
    Queued,
    Running,
    Completed,
    RetryWait,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "craft_operation_kind", rename_all = "snake_case")]
pub enum CraftOperationKind {
    StageTransfer,
    DependencyTransfer,
    Craft,
    CleanupTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "craft_operation_state", rename_all = "snake_case")]
pub enum CraftOperationState {
    Queued,
    Claimed,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

macro_rules! enum_name {
    ($type:ty, { $($variant:path => $name:literal),+ $(,)? }) => {
        impl $type {
            pub fn as_str(self) -> &'static str {
                match self { $($variant => $name),+ }
            }
        }
    };
}

enum_name!(CraftNodeKind, {
    CraftNodeKind::Recipe => "recipe",
    CraftNodeKind::Storage => "storage",
    CraftNodeKind::Any => "any",
});
enum_name!(CraftNodeState, {
    CraftNodeState::Pending => "pending",
    CraftNodeState::Blocked => "blocked",
    CraftNodeState::Staging => "staging",
    CraftNodeState::Queued => "queued",
    CraftNodeState::Running => "running",
    CraftNodeState::Completed => "completed",
    CraftNodeState::RetryWait => "retry_wait",
    CraftNodeState::Failed => "failed",
    CraftNodeState::Cancelled => "cancelled",
});
enum_name!(CraftOperationKind, {
    CraftOperationKind::StageTransfer => "stage_transfer",
    CraftOperationKind::DependencyTransfer => "dependency_transfer",
    CraftOperationKind::Craft => "craft",
    CraftOperationKind::CleanupTransfer => "cleanup_transfer",
});
enum_name!(CraftOperationState, {
    CraftOperationState::Queued => "queued",
    CraftOperationState::Claimed => "claimed",
    CraftOperationState::Running => "running",
    CraftOperationState::Completed => "completed",
    CraftOperationState::Failed => "failed",
    CraftOperationState::Cancelled => "cancelled",
    CraftOperationState::Interrupted => "interrupted",
});

#[derive(Debug, sqlx::FromRow)]
struct CraftJobRow {
    id: Uuid,
    target_item_kind: String,
    target_quantity: i32,
    selections: serde_json::Value,
    state: CraftJobState,
    completed_quantity: i32,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CraftNodeSpec {
    pub id: Uuid,
    pub key: String,
    pub item_kind: String,
    pub required_quantity: u32,
}

#[derive(Debug, Clone)]
pub struct NewCraftJobNode {
    pub spec: CraftNodeSpec,
    pub parent_id: Option<Uuid>,
    pub child_order: i32,
    pub kind: NewCraftJobNodeKind,
}

#[derive(Debug, Clone)]
pub enum NewCraftJobNodeKind {
    Recipe(CraftRecipeSnapshot),
    Storage,
    Any,
}

#[derive(Debug, Clone)]
pub struct CraftRecipeSnapshot {
    pub recipe_id: Uuid,
    pub engine_type: CraftingEngineType,
    pub ingredients: Vec<CraftIngredient>,
    pub planned_crafts: u32,
    pub output_yield: u32,
}

#[derive(Debug, Clone)]
pub struct ReadyCraftNode {
    pub spec: CraftNodeSpec,
    pub job_id: Uuid,
    pub recipe: CraftRecipeSnapshot,
    pub completed_crafts: u32,
    pub assigned_region_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct ReadyCraftNodeRow {
    id: Uuid,
    job_id: Uuid,
    key: String,
    item_kind: String,
    required_quantity: i32,
    recipe_id: Uuid,
    engine_type: CraftingEngineType,
    recipe_ingredients: serde_json::Value,
    planned_crafts: i32,
    output_yield: i32,
    completed_crafts: i32,
    assigned_region_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct CraftItemRequirement {
    pub item_kind: String,
    pub quantity: u32,
}

#[derive(Debug, Clone)]
pub struct CraftDependencyRequirement {
    pub source_node_id: Uuid,
    pub region_id: Uuid,
    pub requirement: CraftItemRequirement,
}

#[derive(sqlx::FromRow)]
struct CraftItemRequirementRow {
    item_kind: String,
    quantity: i32,
}

#[derive(sqlx::FromRow)]
struct CraftDependencyRequirementRow {
    source_node_id: Uuid,
    item_kind: String,
    quantity: i32,
    region_id: Uuid,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CleanupCraftNode {
    pub id: Uuid,
    pub assigned_region_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CraftJobNodeRecord {
    pub spec: CraftNodeSpec,
    pub parent_id: Option<Uuid>,
    pub child_order: i32,
    pub completed_quantity: i32,
    pub state: CraftNodeState,
    pub assigned_region_id: Option<Uuid>,
    pub error: Option<String>,
    pub kind: CraftJobNodeData,
    pub operation: Option<CraftJobOperationRecord>,
}

#[derive(Debug, Clone)]
pub enum CraftJobNodeData {
    Recipe {
        recipe: CraftRecipeSnapshot,
        completed_crafts: u32,
    },
    Storage,
    Any,
}

#[derive(Debug, Clone)]
pub struct CraftJobOperationRecord {
    pub kind: CraftOperationKind,
    pub state: CraftOperationState,
    pub completed_amount: i32,
    pub error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CraftJobNodeRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    key: String,
    child_order: i32,
    kind: CraftNodeKind,
    item_kind: String,
    required_quantity: i32,
    completed_quantity: i32,
    state: CraftNodeState,
    recipe_id: Option<Uuid>,
    engine_type: Option<CraftingEngineType>,
    planned_crafts: Option<i32>,
    completed_crafts: i32,
    output_yield: Option<i32>,
    recipe_ingredients: Option<serde_json::Value>,
    assigned_region_id: Option<Uuid>,
    error: Option<String>,
    operation_kind: Option<CraftOperationKind>,
    operation_state: Option<CraftOperationState>,
    operation_completed_amount: Option<i32>,
    operation_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCraftTransferOperation {
    pub work_unit: WorkUnit,
    pub kind: NewCraftTransferKind,
}

#[derive(Debug, Clone, Copy)]
pub enum NewCraftTransferKind {
    Stage,
    Dependency { source_node_id: Uuid },
    Cleanup,
}

#[derive(Debug)]
pub struct DispatchableCraftOperation {
    pub kind: CraftOperationKind,
    pub work_unit: WorkUnit,
}

#[derive(sqlx::FromRow)]
struct DispatchableCraftOperationRow {
    kind: CraftOperationKind,
    payload: serde_json::Value,
}

impl TryFrom<CraftJobNodeRow> for CraftJobNodeRecord {
    type Error = sqlx::Error;

    fn try_from(row: CraftJobNodeRow) -> Result<Self, Self::Error> {
        let node_kind = row.kind;
        let node_key = row.key.clone();
        let missing = move |field: &str| {
            sqlx::Error::Protocol(
                format!("{} node {} has no {field}", node_kind.as_str(), node_key).into(),
            )
        };
        let kind = match row.kind {
            CraftNodeKind::Recipe => CraftJobNodeData::Recipe {
                recipe: CraftRecipeSnapshot {
                    recipe_id: row.recipe_id.ok_or_else(|| missing("recipe id"))?,
                    engine_type: row.engine_type.ok_or_else(|| missing("engine type"))?,
                    ingredients: serde_json::from_value(
                        row.recipe_ingredients
                            .clone()
                            .ok_or_else(|| missing("recipe ingredients"))?,
                    )
                    .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
                    planned_crafts: u32::try_from(
                        row.planned_crafts
                            .ok_or_else(|| missing("planned crafts"))?,
                    )
                    .map_err(|_| missing("valid planned crafts"))?,
                    output_yield: u32::try_from(
                        row.output_yield.ok_or_else(|| missing("output yield"))?,
                    )
                    .map_err(|_| missing("valid output yield"))?,
                },
                completed_crafts: u32::try_from(row.completed_crafts)
                    .map_err(|_| missing("valid completed crafts"))?,
            },
            CraftNodeKind::Storage => CraftJobNodeData::Storage,
            CraftNodeKind::Any => CraftJobNodeData::Any,
        };
        let operation = match (row.operation_kind, row.operation_state) {
            (Some(kind), Some(state)) => Some(CraftJobOperationRecord {
                kind,
                state,
                completed_amount: row.operation_completed_amount.unwrap_or_default(),
                error: row.operation_error,
            }),
            (None, None) => None,
            _ => {
                return Err(sqlx::Error::Protocol(
                    "incomplete craft operation row".into(),
                ));
            }
        };
        Ok(Self {
            spec: CraftNodeSpec {
                id: row.id,
                key: row.key,
                item_kind: row.item_kind,
                required_quantity: u32::try_from(row.required_quantity)
                    .map_err(|_| missing("valid required quantity"))?,
            },
            parent_id: row.parent_id,
            child_order: row.child_order,
            completed_quantity: row.completed_quantity,
            state: row.state,
            assigned_region_id: row.assigned_region_id,
            error: row.error,
            kind,
            operation,
        })
    }
}

impl super::DatabaseHandle {
    pub async fn prepare_craft_recovery(&self) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query!("update craft_processing_lease set indexed_after_startup = false")
            .execute(&mut *transaction)
            .await?;
        sqlx::query!(
            r#"update craft_job_operation
               set state = 'interrupted', updated_at = now()
               where state in ('queued', 'claimed', 'running')"#,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }

    pub async fn complete_craft_recovery(&self) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query!("update craft_processing_lease set indexed_after_startup = true")
            .execute(&mut *transaction)
            .await?;
        sqlx::query!(
            r#"update craft_job_operation
               set state = 'queued',
                   payload = case
                       when checkpoint <> '{}'::jsonb then checkpoint
                       else payload
                   end,
                   updated_at = now()
               where state = 'interrupted'"#,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }

    pub async fn create_craft_job(
        &self,
        status: &CraftJobStatus,
        nodes: &[NewCraftJobNode],
    ) -> Result<(), anyhow::Error> {
        let id = Uuid::parse_str(&status.id)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query!(
            "insert into craft_job (id, target_item_kind, target_quantity, selections, state, completed_quantity, error) values ($1, $2, $3, $4, $5, $6, $7)",
            id,
            &status.target_item_kind,
            i32::try_from(status.target_quantity).map_err(|_| sqlx::Error::Protocol("craft quantity is too large".into()))?,
            serde_json::to_value(&status.selections).map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
            status.state as CraftJobState,
            i32::try_from(status.completed_quantity).map_err(|_| sqlx::Error::Protocol("completed quantity is too large".into()))?,
            status.error.as_deref(),
        )
        .execute(&mut *transaction)
        .await?;

        for node in nodes {
            let (kind, recipe_id, engine_type, ingredients, planned_crafts, output_yield) =
                match &node.kind {
                    NewCraftJobNodeKind::Recipe(recipe) => (
                        CraftNodeKind::Recipe,
                        Some(recipe.recipe_id),
                        Some(recipe.engine_type),
                        Some(&recipe.ingredients),
                        Some(recipe.planned_crafts),
                        Some(recipe.output_yield),
                    ),
                    NewCraftJobNodeKind::Storage => {
                        (CraftNodeKind::Storage, None, None, None, None, None)
                    }
                    NewCraftJobNodeKind::Any => (CraftNodeKind::Any, None, None, None, None, None),
                };
            let required_quantity = i32::try_from(node.spec.required_quantity)
                .map_err(|_| anyhow::anyhow!("craft node quantity is too large"))?;
            let planned_crafts = planned_crafts
                .map(i32::try_from)
                .transpose()
                .map_err(|_| anyhow::anyhow!("craft count is too large"))?;
            let output_yield = output_yield
                .map(i32::try_from)
                .transpose()
                .map_err(|_| anyhow::anyhow!("craft output yield is too large"))?;
            let recipe_ingredients = ingredients.map(serde_json::to_value).transpose()?;
            sqlx::query!(
                r#"insert into craft_job_node (
                    id, job_id, parent_id, key, child_order, kind, item_kind,
                    required_quantity, recipe_id, engine_type, recipe_ingredients,
                    planned_crafts, output_yield
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
                node.spec.id,
                id,
                node.parent_id,
                &node.spec.key,
                node.child_order,
                kind as CraftNodeKind,
                &node.spec.item_kind,
                required_quantity,
                recipe_id,
                engine_type as Option<CraftingEngineType>,
                recipe_ingredients,
                planned_crafts,
                output_yield,
            )
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub async fn fetch_craft_jobs(&self) -> Result<Vec<CraftJobStatus>, sqlx::Error> {
        let rows = sqlx::query_as!(
            CraftJobRow,
            "select id, target_item_kind, target_quantity, selections, state as \"state: CraftJobState\", completed_quantity, error from craft_job order by created_at desc",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(craft_job_status).collect()
    }

    pub async fn fetch_craft_job(&self, id: Uuid) -> Result<Option<CraftJobStatus>, sqlx::Error> {
        let row = sqlx::query_as!(
            CraftJobRow,
            "select id, target_item_kind, target_quantity, selections, state as \"state: CraftJobState\", completed_quantity, error from craft_job where id = $1",
            id,
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(craft_job_status).transpose()
    }

    pub async fn fetch_craft_job_nodes(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<CraftJobNodeRecord>, sqlx::Error> {
        let rows = sqlx::query_as!(
            CraftJobNodeRow,
            r#"select /* enum-backed craft node */
                   node.id,
                   node.parent_id,
                   node.key,
                   node.child_order,
                   node.kind as "kind: CraftNodeKind",
                   node.item_kind,
                   node.required_quantity,
                   node.completed_quantity,
                   node.state as "state: CraftNodeState",
                   node.recipe_id,
                   node.engine_type as "engine_type: CraftingEngineType",
                   node.planned_crafts,
                   node.completed_crafts,
                   node.output_yield,
                   node.recipe_ingredients,
                   node.assigned_region_id,
                   node.error,
                   operation.kind as "operation_kind?: CraftOperationKind",
                   operation.state as "operation_state?: CraftOperationState",
                   operation.completed_amount as operation_completed_amount,
                   operation.error as operation_error
               from craft_job_node as node
               left join lateral (
                   select kind, state, completed_amount, error
                   from craft_job_operation
                   where node_id = node.id
                   order by created_at desc, sequence desc
                   limit 1
               ) as operation on true
               where node.job_id = $1
               order by node.key, node.child_order"#,
            job_id,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn reconcile_ready_craft_nodes(&self) -> Result<Vec<ReadyCraftNode>, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query!(
            r#"update craft_job_node as node
               set state = case
                       when exists (
                           select 1
                           from craft_job_node as child
                           where child.parent_id = node.id
                             and child.kind = 'recipe'
                             and child.state <> 'completed'
                       ) then 'blocked'::craft_node_state
                       else 'pending'::craft_node_state
                   end,
                   updated_at = now()
               from craft_job
               where craft_job.id = node.job_id
                 and craft_job.state in ('queued', 'waiting', 'running')
                 and node.kind = 'recipe'
                 and node.state in ('pending', 'blocked')"#,
        )
        .execute(&mut *transaction)
        .await?;

        let nodes = sqlx::query_as!(
            ReadyCraftNodeRow,
            r#"select
                   node.id,
                   node.job_id,
                   node.key,
                   node.item_kind,
                   node.required_quantity,
                   node.recipe_id as "recipe_id!",
                   node.engine_type as "engine_type!: CraftingEngineType",
                   node.recipe_ingredients as "recipe_ingredients!",
                   node.planned_crafts as "planned_crafts!",
                   node.output_yield as "output_yield!",
                   node.completed_crafts,
                   node.assigned_region_id
               from craft_job_node as node
               join craft_job on craft_job.id = node.job_id
               where craft_job.state in ('queued', 'waiting', 'running')
                 and node.kind = 'recipe'
                 and node.state = 'pending'
                 and (node.retry_at is null or node.retry_at <= now())
               order by craft_job.created_at, node.key"#,
        )
        .fetch_all(&mut *transaction)
        .await?;

        transaction.commit().await?;
        nodes.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn fetch_craft_material_requirements(
        &self,
        node_id: Uuid,
    ) -> Result<Vec<CraftItemRequirement>, sqlx::Error> {
        let rows = sqlx::query_as!(
            CraftItemRequirementRow,
            r#"select item_kind, sum(required_quantity)::integer as "quantity!"
               from craft_job_node
               where parent_id = $1
                 and kind in ('storage', 'any')
               group by item_kind
               order by item_kind"#,
            node_id,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(CraftItemRequirement {
                    item_kind: row.item_kind,
                    quantity: u32::try_from(row.quantity).map_err(|_| {
                        sqlx::Error::Protocol("material quantity cannot be negative".into())
                    })?,
                })
            })
            .collect()
    }

    pub async fn fetch_craft_dependency_requirements(
        &self,
        node_id: Uuid,
    ) -> Result<Vec<CraftDependencyRequirement>, sqlx::Error> {
        let rows = sqlx::query_as!(
            CraftDependencyRequirementRow,
            r#"select
                   id as "source_node_id!",
                   item_kind,
                   required_quantity as quantity,
                   assigned_region_id as "region_id!"
               from craft_job_node
               where parent_id = $1
                 and kind = 'recipe'
                 and state = 'completed'
                 and assigned_region_id is not null
               order by child_order"#,
            node_id,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(CraftDependencyRequirement {
                    source_node_id: row.source_node_id,
                    region_id: row.region_id,
                    requirement: CraftItemRequirement {
                        item_kind: row.item_kind,
                        quantity: u32::try_from(row.quantity).map_err(|_| {
                            sqlx::Error::Protocol("dependency quantity cannot be negative".into())
                        })?,
                    },
                })
            })
            .collect()
    }

    pub async fn fetch_queued_craft_nodes(&self) -> Result<Vec<ReadyCraftNode>, sqlx::Error> {
        let nodes = sqlx::query_as!(
            ReadyCraftNodeRow,
            r#"select
                   node.id,
                   node.job_id,
                   node.key,
                   node.item_kind,
                   node.required_quantity,
                   node.recipe_id as "recipe_id!",
                   node.engine_type as "engine_type!: CraftingEngineType",
                   node.recipe_ingredients as "recipe_ingredients!",
                   node.planned_crafts as "planned_crafts!",
                   node.output_yield as "output_yield!",
                   node.completed_crafts,
                   node.assigned_region_id
               from craft_job_node as node
               join craft_job on craft_job.id = node.job_id
               where craft_job.state in ('queued', 'waiting', 'running')
                 and node.kind = 'recipe'
                 and node.state = 'queued'
               order by craft_job.created_at, node.key"#,
        )
        .fetch_all(&self.pool)
        .await?;
        nodes.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn fetch_cleanup_craft_nodes(&self) -> Result<Vec<CleanupCraftNode>, sqlx::Error> {
        sqlx::query_as!(
            CleanupCraftNode,
            r#"select node.id, node.assigned_region_id as "assigned_region_id!"
               from craft_job_node as node
               join craft_job on craft_job.id = node.job_id
               where node.parent_id is null
                 and node.state = 'completed'
                 and node.assigned_region_id is not null
                 and craft_job.state = 'running'
                 and not exists (
                     select 1 from craft_job_operation
                     where node_id = node.id and kind = 'cleanup_transfer'
                 )
               order by craft_job.created_at"#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn persist_craft_work_unit_progress(
        &self,
        work_unit: &WorkUnit,
    ) -> Result<bool, sqlx::Error> {
        let WorkUnitData::Craft(craft) = &work_unit.data else {
            return Ok(false);
        };
        let completed_crafts = i32::try_from(craft.progress.completed_crafts)
            .map_err(|_| sqlx::Error::Protocol("completed craft count is too large".into()))?;
        let checkpoint = serde_json::to_value(work_unit)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
        let is_done = work_unit.is_done();
        let mut transaction = self.pool.begin().await?;
        let operation = sqlx::query!(
            r#"update craft_job_operation
               set completed_amount = $2,
                   checkpoint = $3,
                   state = case when $4 then 'completed'::craft_operation_state else 'running'::craft_operation_state end,
                   updated_at = now(),
                   completed_at = case when $4 then now() else null end
               where work_unit_id = $1
                 and kind = 'craft'
                 and state in ('queued', 'claimed', 'running')
                 and completed_amount <= $2
               returning node_id"#,
            work_unit.id as WorkUnitId,
            completed_crafts,
            checkpoint,
            is_done,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(operation) = operation else {
            transaction.rollback().await?;
            return Ok(false);
        };

        sqlx::query!(
            r#"update craft_job_node
               set completed_crafts = $2,
                   completed_quantity = least(required_quantity, $2 * output_yield),
                   state = case when $3 then 'completed'::craft_node_state else 'running'::craft_node_state end,
                   updated_at = now()
               where id = $1
                 and completed_crafts <= $2"#,
            operation.node_id,
            completed_crafts,
            is_done,
        )
        .execute(&mut *transaction)
        .await?;

        sqlx::query!(
            r#"update craft_job
               set completed_quantity = root.completed_quantity,
                   state = 'running',
                   updated_at = now()
               from craft_job_node as root
               where root.id = $1
                 and root.parent_id is null
                 and craft_job.id = root.job_id"#,
            operation.node_id,
        )
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    pub async fn register_cleanup_operations(
        &self,
        node_id: Uuid,
        operations: &[NewCraftTransferOperation],
    ) -> Result<bool, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let locked = sqlx::query_scalar!(
            "select id from craft_job_node where id = $1 and parent_id is null and state = 'completed' for update",
            node_id,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if locked.is_none() {
            transaction.rollback().await?;
            return Ok(false);
        }
        let has_cleanup = sqlx::query_scalar!(
            r#"select exists (
                   select 1 from craft_job_operation
                   where node_id = $1 and kind = 'cleanup_transfer'
               ) as "exists!""#,
            node_id,
        )
        .fetch_one(&mut *transaction)
        .await?;
        if has_cleanup {
            transaction.rollback().await?;
            return Ok(false);
        }
        for (sequence, operation) in operations.iter().enumerate() {
            if !matches!(operation.kind, NewCraftTransferKind::Cleanup) {
                return Err(sqlx::Error::Protocol(
                    "non-cleanup operation passed to cleanup registration".into(),
                ));
            }
            let payload = serde_json::to_value(&operation.work_unit)
                .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
            sqlx::query!(
                r#"insert into craft_job_operation (
                       id, node_id, work_unit_id, kind, attempt, sequence, payload
                   ) values ($1, $2, $3, 'cleanup_transfer', 1, $4, $5)"#,
                Uuid::new_v4(),
                node_id,
                operation.work_unit.id as WorkUnitId,
                i32::try_from(sequence)
                    .map_err(|_| sqlx::Error::Protocol("too many cleanup operations".into()))?,
                payload,
            )
            .execute(&mut *transaction)
            .await?;
        }
        if operations.is_empty() {
            sqlx::query!(
                "delete from craft_processing_lease where node_id = $1",
                node_id,
            )
            .execute(&mut *transaction)
            .await?;
            sqlx::query!(
                "update craft_job_node set assigned_region_id = null, updated_at = now() where id = $1",
                node_id,
            )
            .execute(&mut *transaction)
            .await?;
            sqlx::query!(
                "update craft_job set state = 'completed', updated_at = now() where id = (select job_id from craft_job_node where id = $1)",
                node_id,
            )
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn register_craft_staging_operations(
        &self,
        node_id: Uuid,
        operations: &[NewCraftTransferOperation],
    ) -> Result<bool, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let locked = sqlx::query_scalar!(
            r#"select id
                             from craft_job_node
                             where id = $1
                                 and state in ('pending', 'staging')
                             for update"#,
            node_id,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if locked.is_none() {
            transaction.rollback().await?;
            return Ok(false);
        }

        let has_operations = sqlx::query_scalar!(
            r#"select exists (
                   select 1
                   from craft_job_operation
                   where node_id = $1
                     and kind in ('stage_transfer', 'dependency_transfer')
                     and state in ('queued', 'claimed', 'running')
               ) as "exists!""#,
            node_id,
        )
        .fetch_one(&mut *transaction)
        .await?;
        if has_operations {
            transaction.rollback().await?;
            return Ok(false);
        }

        for (sequence, operation) in operations.iter().enumerate() {
            let payload = serde_json::to_value(&operation.work_unit)
                .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
            let (kind, source_node_id) = match operation.kind {
                NewCraftTransferKind::Stage => (CraftOperationKind::StageTransfer, None),
                NewCraftTransferKind::Dependency { source_node_id } => {
                    (CraftOperationKind::DependencyTransfer, Some(source_node_id))
                }
                NewCraftTransferKind::Cleanup => {
                    return Err(sqlx::Error::Protocol(
                        "cleanup operation passed to staging registration".into(),
                    ));
                }
            };
            sqlx::query!(
                r#"insert into craft_job_operation (
                       id, node_id, source_node_id, work_unit_id, kind, attempt, sequence, payload
                   ) values ($1, $2, $3, $4, $5, 1, $6, $7)"#,
                Uuid::new_v4(),
                node_id,
                source_node_id,
                operation.work_unit.id as WorkUnitId,
                kind as CraftOperationKind,
                i32::try_from(sequence)
                    .map_err(|_| sqlx::Error::Protocol("too many staging operations".into()))?,
                payload,
            )
            .execute(&mut *transaction)
            .await?;
        }

        let state = if operations.is_empty() {
            CraftNodeState::Queued
        } else {
            CraftNodeState::Staging
        };
        sqlx::query!(
            "update craft_job_node set state = $2, updated_at = now() where id = $1",
            node_id,
            state as CraftNodeState,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn register_craft_operation(
        &self,
        node_id: Uuid,
        work_unit: &WorkUnit,
    ) -> Result<bool, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let locked = sqlx::query_scalar!(
            "select id from craft_job_node where id = $1 and state = 'queued' for update",
            node_id,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if locked.is_none() {
            transaction.rollback().await?;
            return Ok(false);
        }
        let has_operation = sqlx::query_scalar!(
            r#"select exists (
                   select 1 from craft_job_operation
                   where node_id = $1 and kind = 'craft'
                     and state in ('queued', 'claimed', 'running')
               ) as "exists!""#,
            node_id,
        )
        .fetch_one(&mut *transaction)
        .await?;
        if has_operation {
            transaction.rollback().await?;
            return Ok(false);
        }
        let attempt = sqlx::query_scalar!(
            "select coalesce(max(attempt), 0)::integer + 1 as \"attempt!\" from craft_job_operation where node_id = $1 and kind = 'craft'",
            node_id,
        )
        .fetch_one(&mut *transaction)
        .await?;
        let payload = serde_json::to_value(work_unit)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
        sqlx::query!(
            r#"insert into craft_job_operation (
                   id, node_id, work_unit_id, kind, attempt, sequence, payload
               ) values ($1, $2, $3, 'craft', $4, 0, $5)"#,
            Uuid::new_v4(),
            node_id,
            work_unit.id as WorkUnitId,
            attempt,
            payload,
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query!(
            "update craft_job_node set state = 'running', updated_at = now() where id = $1",
            node_id,
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query!(
            "update craft_job set state = 'running', updated_at = now() where id = (select job_id from craft_job_node where id = $1)",
            node_id,
        )
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn fetch_dispatchable_craft_operations(
        &self,
    ) -> Result<Vec<DispatchableCraftOperation>, sqlx::Error> {
        let rows = sqlx::query_as!(
            DispatchableCraftOperationRow,
            r#"select operation.kind as "kind: CraftOperationKind", operation.payload
               from craft_job_operation as operation
               join craft_job_node as node on node.id = operation.node_id
               join craft_job on craft_job.id = node.job_id
               where operation.state = 'queued'
                 and craft_job.state in ('queued', 'waiting', 'running')
                 and (
                     operation.kind = 'craft'
                     or not exists (
                         select 1
                         from craft_job_operation as predecessor
                         where predecessor.node_id = operation.node_id
                           and predecessor.sequence < operation.sequence
                           and predecessor.state <> 'completed'
                           and (
                               (operation.kind in ('stage_transfer', 'dependency_transfer')
                                   and predecessor.kind in ('stage_transfer', 'dependency_transfer'))
                               or predecessor.kind = operation.kind
                           )
                     )
                 )
               order by craft_job.created_at, operation.created_at, operation.sequence"#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DispatchableCraftOperation {
                    kind: row.kind,
                    work_unit: serde_json::from_value(row.payload)
                        .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
                })
            })
            .collect()
    }

    pub async fn has_active_craft_transfer_operations(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"select exists (
                   select 1 from craft_job_operation
                   where kind in ('stage_transfer', 'dependency_transfer', 'cleanup_transfer')
                     and state in ('queued', 'claimed', 'running')
               ) as "exists!""#,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn mark_craft_operation_claimed(
        &self,
        work_unit_id: WorkUnitId,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"update craft_job_operation
               set state = 'claimed', updated_at = now()
               where work_unit_id = $1 and state = 'queued'"#,
            work_unit_id as WorkUnitId,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn persist_transfer_work_unit_progress(
        &self,
        work_unit: &WorkUnit,
    ) -> Result<bool, sqlx::Error> {
        let WorkUnitData::Transfer(transfer) = &work_unit.data else {
            return Ok(false);
        };
        let completed_amount = i32::try_from(transfer.completed.len())
            .map_err(|_| sqlx::Error::Protocol("completed transfer count is too large".into()))?;
        let checkpoint = serde_json::to_value(work_unit)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
        let is_done = work_unit.is_done();
        let mut transaction = self.pool.begin().await?;
        let operation = sqlx::query!(
            r#"update craft_job_operation
               set completed_amount = $2,
                   checkpoint = $3,
                   state = case when $4 then 'completed'::craft_operation_state else 'running'::craft_operation_state end,
                   updated_at = now(),
                   completed_at = case when $4 then now() else null end
               where work_unit_id = $1
                 and kind in ('stage_transfer', 'dependency_transfer', 'cleanup_transfer')
                 and state in ('queued', 'claimed', 'running')
                 and completed_amount <= $2
               returning node_id, kind as "kind: CraftOperationKind""#,
            work_unit.id as WorkUnitId,
            completed_amount,
            checkpoint,
            is_done,
        )
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(operation) = operation else {
            transaction.rollback().await?;
            return Ok(false);
        };

        if is_done && operation.kind == CraftOperationKind::CleanupTransfer {
            let all_cleaned = sqlx::query_scalar!(
                r#"select not exists (
                       select 1 from craft_job_operation
                       where node_id = $1 and kind = 'cleanup_transfer' and state <> 'completed'
                   ) as "all_cleaned!""#,
                operation.node_id,
            )
            .fetch_one(&mut *transaction)
            .await?;
            if all_cleaned {
                sqlx::query!(
                    "delete from craft_processing_lease where node_id = $1",
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
                sqlx::query!(
                    "update craft_job_node set assigned_region_id = null, updated_at = now() where id = $1",
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
                sqlx::query!(
                    "update craft_job set state = 'completed', updated_at = now() where id = (select job_id from craft_job_node where id = $1)",
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
            }
        } else if is_done {
            let all_staged = sqlx::query_scalar!(
                r#"select not exists (
                       select 1
                       from craft_job_operation
                       where node_id = $1
                         and kind in ('stage_transfer', 'dependency_transfer')
                         and state <> 'completed'
                   ) as "all_staged!""#,
                operation.node_id,
            )
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query!(
                r#"update craft_job_node
                   set state = case when $2 then 'queued'::craft_node_state else 'staging'::craft_node_state end,
                   updated_at = now()
                   where id = $1"#,
                operation.node_id,
                all_staged,
            )
            .execute(&mut *transaction)
            .await?;
            if all_staged {
                sqlx::query!(
                    r#"update craft_job_node
                       set completed_quantity = required_quantity,
                           state = 'completed',
                           updated_at = now()
                       where parent_id = $1 and kind in ('storage', 'any')"#,
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
                sqlx::query!(
                    r#"delete from craft_processing_lease
                       where node_id in (
                           select distinct source_node_id
                           from craft_job_operation
                           where node_id = $1 and source_node_id is not null
                       )"#,
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
                sqlx::query!(
                    r#"update craft_job_node
                       set assigned_region_id = null, updated_at = now()
                       where id in (
                           select distinct source_node_id
                           from craft_job_operation
                           where node_id = $1 and source_node_id is not null
                       )"#,
                    operation.node_id,
                )
                .execute(&mut *transaction)
                .await?;
            }
        }

        transaction.commit().await?;
        Ok(true)
    }
}

impl TryFrom<ReadyCraftNodeRow> for ReadyCraftNode {
    type Error = sqlx::Error;

    fn try_from(row: ReadyCraftNodeRow) -> Result<Self, Self::Error> {
        Ok(Self {
            spec: CraftNodeSpec {
                id: row.id,
                key: row.key,
                item_kind: row.item_kind,
                required_quantity: u32::try_from(row.required_quantity).map_err(|_| {
                    sqlx::Error::Protocol("craft node quantity cannot be negative".into())
                })?,
            },
            job_id: row.job_id,
            recipe: CraftRecipeSnapshot {
                recipe_id: row.recipe_id,
                engine_type: row.engine_type,
                ingredients: serde_json::from_value(row.recipe_ingredients)
                    .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
                planned_crafts: u32::try_from(row.planned_crafts).map_err(|_| {
                    sqlx::Error::Protocol("planned crafts cannot be negative".into())
                })?,
                output_yield: u32::try_from(row.output_yield)
                    .map_err(|_| sqlx::Error::Protocol("output yield cannot be negative".into()))?,
            },
            completed_crafts: u32::try_from(row.completed_crafts)
                .map_err(|_| sqlx::Error::Protocol("completed crafts cannot be negative".into()))?,
            assigned_region_id: row.assigned_region_id,
        })
    }
}

fn craft_job_status(row: CraftJobRow) -> Result<CraftJobStatus, sqlx::Error> {
    Ok(CraftJobStatus {
        id: row.id.to_string(),
        target_item_kind: row.target_item_kind,
        target_quantity: u32::try_from(row.target_quantity).map_err(|_| {
            sqlx::Error::Protocol("craft target quantity cannot be negative".into())
        })?,
        selections: serde_json::from_value(row.selections)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
        state: row.state,
        completed_quantity: u32::try_from(row.completed_quantity)
            .map_err(|_| sqlx::Error::Protocol("completed quantity cannot be negative".into()))?,
        error: row.error,
    })
}
