use std::{hash::{DefaultHasher, Hash, Hasher}, str::FromStr};

use azalea_inventory::item::MaxStackSizeExt as _;
use azalea_registry::builtin::ItemKind;
use serde::{Deserialize, Serialize};
use sqlx::types::{JsonValue, Uuid};

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: Uuid,
    pub container_id: Uuid,
    pub item_kind: String,
    pub slot: i32,
    pub components: JsonValue,
    pub quantity: i32,
    pub components_digest: String,
}

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SKU(u64, i32);

impl ItemStack {
    pub fn sku(&self) -> SKU {
        let mut hasher = DefaultHasher::new();
        self.item_kind.hash(&mut hasher);
        self.components_digest.hash(&mut hasher);

        let stack_size = match ItemKind::from_str(&self.item_kind) {
            Ok(item_kind) => item_kind.max_stack_size(),
            Err(_) => 64,
        };

        SKU(hasher.finish(), stack_size)
    }
}


impl SKU {
    pub fn stack_size(&self) -> i32 {
        self.1
    }
}