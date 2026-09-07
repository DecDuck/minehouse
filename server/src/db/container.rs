use std::collections::HashMap;
use std::str::FromStr;

use azalea_registry::builtin::ItemKind;
use common::item_stack::ItemStack;
use sqlx::types::Uuid;

use super::Cube;
use crate::mwms::category::{ItemCategory, ItemCategoryProfile, ItemCategoryProfiles};

#[derive(sqlx::FromRow)]
struct ContainerRow {
    id: Uuid,
    region_id: Uuid,
    position: Cube,
    capacity: i32,
}

#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct ContainerLocation {
    pub id: Uuid,
    pub position: Cube,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Container {
    pub id: Uuid,
    pub region_id: Uuid,
    pub position: Cube,
    pub capacity: u64,

    // Slot position to itemstack
    pub contents: HashMap<usize, ItemStack>,
}

impl super::DatabaseHandle {
    pub async fn fetch_all_container_locations(
        &self,
    ) -> Result<Vec<ContainerLocation>, sqlx::Error> {
        sqlx::query_as!(
            ContainerLocation,
            r#"select id, position as "position: Cube" from container"#,
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn fetch_all_containers(&self) -> Result<Vec<Container>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ContainerRow>(
            "select id, region_id, position, capacity from container",
        )
        .fetch_all(&self.pool)
        .await?;
        let stacks = sqlx::query_as::<_, ItemStack>(
            "select id, container_id, item_kind, slot, components, quantity, components_digest \
             from item_stack",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut contents: HashMap<Uuid, HashMap<usize, ItemStack>> = HashMap::new();
        for stack in stacks {
            let slot = usize::try_from(stack.slot)
                .map_err(|_| sqlx::Error::Protocol("item stack slot cannot be negative".into()))?;
            contents
                .entry(stack.container_id)
                .or_default()
                .insert(slot, stack);
        }

        rows.into_iter()
            .map(|row| {
                let capacity = u64::try_from(row.capacity).map_err(|_| {
                    sqlx::Error::Protocol("container capacity cannot be negative".into())
                })?;
                Ok(Container {
                    id: row.id,
                    region_id: row.region_id,
                    position: row.position,
                    capacity,
                    contents: contents.remove(&row.id).unwrap_or_default(),
                })
            })
            .collect()
    }
}

impl Container {
    /// Picks the category this container is for by tallying the categories of
    /// its contents (weighted by quantity) and taking the largest. Empty
    /// containers stay unallocated.
    pub fn categorize(&self, profile: &ItemCategoryProfiles) -> Option<ItemCategory> {
        let mut tally: HashMap<ItemCategory, u64> = HashMap::new();
        for stack in self.contents.values() {
            let Ok(kind) = ItemKind::from_str(&stack.item_kind) else {
                continue;
            };
            let category = profile.map_item_kind(&kind);
            *tally.entry(category).or_insert(0) += stack.quantity.max(0) as u64;
        }
        tally
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(category, _)| category)
    }
}
