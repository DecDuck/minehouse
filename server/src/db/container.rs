use std::collections::HashMap;
use std::str::FromStr;

use azalea_registry::builtin::ItemKind;
use common::item_stack::ItemStack;
use sqlx::types::Uuid;

use super::Cube;
use crate::mwms::category::{ItemCategory, ItemCategoryProfile, ItemCategoryProfiles};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Container {
    pub id: Uuid,
    pub region_id: Uuid,
    pub position: Cube,
    pub capacity: u64,

    // Slot position to itemstack
    pub contents: HashMap<usize, ItemStack>,
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
