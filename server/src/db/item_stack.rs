use common::item_stack::ItemStack;
use sqlx::types::{JsonValue, Uuid};

use super::DatabaseHandle;

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
        let item_kinds: Vec<&str> = stacks
            .iter()
            .map(|stack| stack.item_kind.as_str())
            .collect();
        let slots: Vec<i32> = stacks.iter().map(|stack| stack.slot).collect();
        let components: Vec<JsonValue> = stacks
            .iter()
            .map(|stack| stack.components.clone())
            .collect();
        let quantities: Vec<i32> = stacks.iter().map(|stack| stack.quantity).collect();

        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "delete from item_stack where container_id = $1",
            container_id
        )
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
}
