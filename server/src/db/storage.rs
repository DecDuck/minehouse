use sqlx::types::{JsonValue, Uuid};

use super::{Cube, DatabaseHandle};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageItemRow {
    pub item_kind: String,
    pub quantity: i64,
    pub stack_count: i64,
    pub container_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ItemKindStackRow {
    pub stack_id: Uuid,
    pub container_id: Uuid,
    pub region_id: Uuid,
    pub region_type: String,
    pub position: Cube,
    pub slot: i32,
    pub quantity: i32,
    pub components: JsonValue,
    pub components_digest: String,
}

impl DatabaseHandle {
    pub async fn search_storage(
        &self,
        item_query: Option<&str>,
        region_id: Option<Uuid>,
        min_quantity: Option<i64>,
        sort_by: &str,
        sort_direction: &str,
    ) -> Result<Vec<StorageItemRow>, sqlx::Error> {
        sqlx::query_as!(
            StorageItemRow,
            r#"select
                item_stack.item_kind,
                sum(item_stack.quantity)::bigint as "quantity!",
                count(*)::bigint as "stack_count!",
                count(distinct item_stack.container_id)::bigint as "container_count!"
            from item_stack
            join container on container.id = item_stack.container_id
                        where ($1::text is null or lower(replace(item_stack.item_kind, '_', ' '))
                                     like '%' || lower(replace($1, '_', ' ')) || '%')
                            and ($2::uuid is null or container.region_id = $2)
                        group by item_stack.item_kind
                        having ($3::bigint is null or sum(item_stack.quantity)::bigint >= $3)
                        order by
                                case when $4 = 'name' and $5 = 'asc' then item_stack.item_kind end asc,
                                case when $4 = 'name' and $5 = 'desc' then item_stack.item_kind end desc,
                                case when $4 = 'quantity' and $5 = 'asc' then sum(item_stack.quantity) end asc,
                                case when $4 = 'quantity' and $5 = 'desc' then sum(item_stack.quantity) end desc,
                                case when $4 = 'stacks' and $5 = 'asc' then count(*) end asc,
                                case when $4 = 'stacks' and $5 = 'desc' then count(*) end desc,
                                case when $4 = 'containers' and $5 = 'asc' then count(distinct item_stack.container_id) end asc,
                                case when $4 = 'containers' and $5 = 'desc' then count(distinct item_stack.container_id) end desc,
                                item_stack.item_kind"#,
            item_query,
            region_id,
                        min_quantity,
                        sort_by,
                        sort_direction,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn fetch_item_kind_stacks(
        &self,
        item_kind: &str,
    ) -> Result<Vec<ItemKindStackRow>, sqlx::Error> {
        sqlx::query_as!(
            ItemKindStackRow,
            r#"select
                item_stack.id as "stack_id!",
                item_stack.container_id as "container_id!",
                container.region_id as "region_id!",
                container_region.type::text as "region_type!",
                container.position as "position: Cube",
                item_stack.slot,
                item_stack.quantity,
                item_stack.components as "components: JsonValue",
                item_stack.components_digest as "components_digest!"
            from item_stack
            join container on container.id = item_stack.container_id
            join container_region on container_region.id = container.region_id
            where item_stack.item_kind = $1
            order by container_region.type, container.position, item_stack.slot"#,
            item_kind,
        )
        .fetch_all(&self.pool)
        .await
    }
}
