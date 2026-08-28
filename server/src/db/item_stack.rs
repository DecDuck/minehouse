use common::item_stack::ItemStack;
use sqlx::types::{JsonValue, Uuid};

use super::{DatabaseHandle, container_region::RegionType};
use crate::mwms::category::ItemCategory;

impl DatabaseHandle {
    /// Atomically replaces the entire contents of a container: every existing
    /// item stack for `container_id` is deleted and `stacks` is inserted in a
    /// single transaction, so the container is never observed half-updated.
    pub async fn replace_container_item_stacks(
        &self,
        container_id: Uuid,
        stacks: &[ItemStack],
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = stacks.iter().map(|stack| stack.id).collect();
        let item_kinds: Vec<&str> = stacks.iter().map(|stack| stack.item_kind.as_str()).collect();
        let slots: Vec<i32> = stacks.iter().map(|stack| stack.slot).collect();
        let components: Vec<JsonValue> = stacks.iter().map(|stack| stack.components.clone()).collect();
        let quantities: Vec<i32> = stacks.iter().map(|stack| stack.quantity).collect();

        let mut tx = self.pool.begin().await?;

        sqlx::query!("delete from item_stack where container_id = $1", container_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query!(
            "insert into item_stack (id, container_id, item_kind, slot, components, quantity)
             select id, $1, item_kind, slot, components, quantity
             from unnest($2::uuid[], $3::text[], $4::int[], $5::jsonb[], $6::int[])
                 as t(id, item_kind, slot, components, quantity)",
            container_id,
            &ids,
            &item_kinds as &[&str],
            &slots,
            &components,
            &quantities,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await
    }

    pub async fn get_container_contents(
        &self,
        container_id: Uuid,
    ) -> Result<Vec<ItemStack>, sqlx::Error> {
        sqlx::query_as::<_, ItemStack>(
            "select id, container_id, item_kind, slot, components, quantity, components_digest
             from item_stack where container_id = $1",
        )
        .bind(container_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Finds stacks of a given `item_kind` in containers of a region type
    /// (e.g. all bulk stacks of `minecraft:stone`).
    pub async fn find_stacks_in_region_type(
        &self,
        region_type: RegionType,
        item_kind: &str,
    ) -> Result<Vec<ItemStack>, sqlx::Error> {
        sqlx::query_as::<_, ItemStack>(
            "select s.id, s.container_id, s.item_kind, s.slot, s.components, s.quantity, s.components_digest
             from item_stack s
             join container c on c.id = s.container_id
             join container_region r on r.id = c.region_id
             where r.type = $1 and s.item_kind = $2",
        )
        .bind(region_type)
        .bind(item_kind)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn distinct_bulk_item_kinds(
        &self,
        category: ItemCategory,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar::<_, String>(
            "select distinct s.item_kind
             from item_stack s
             join container c on c.id = s.container_id
             join container_region r on r.id = c.region_id
             where r.type = 'bulk' and c.category = $1",
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
    }
}
