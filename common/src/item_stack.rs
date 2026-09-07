use std::{
    hash::{DefaultHasher, Hash, Hasher},
    str::FromStr,
};

use azalea_inventory::item::MaxStackSizeExt as _;
use azalea_registry::builtin::ItemKind;
use serde::{Deserialize, Serialize};
use sqlx::types::{JsonValue, Uuid};

mod components_serde {
    use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer, de, ser};
    use sqlx::types::JsonValue;

    pub fn serialize<S>(value: &JsonValue, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            value.serialize(serializer)
        } else {
            serde_json::to_string(value)
                .map_err(ser::Error::custom)?
                .serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<JsonValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            JsonValue::deserialize(deserializer)
        } else {
            let value = String::deserialize(deserializer)?;
            serde_json::from_str(&value).map_err(de::Error::custom)
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: Uuid,
    pub container_id: Uuid,
    pub item_kind: String,
    pub slot: i32,
    #[serde(with = "components_serde")]
    pub components: JsonValue,
    pub quantity: i32,
    pub components_digest: String,
}

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SKU(u64, i32, ItemKind);

impl ItemStack {
    pub fn sku(&self) -> SKU {
        let mut hasher = DefaultHasher::new();
        self.item_kind.hash(&mut hasher);
        self.components_digest.hash(&mut hasher);

        let (kind, stack_size) = match ItemKind::from_str(&self.item_kind) {
            Ok(kind) => (kind, kind.max_stack_size()),
            Err(_) => (ItemKind::Air, 64),
        };

        SKU(hasher.finish(), stack_size, kind)
    }
}

impl SKU {
    pub fn stack_size(&self) -> i32 {
        self.1
    }

    pub fn kind(&self) -> ItemKind {
        self.2
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ItemStack;

    fn item_stack() -> ItemStack {
        ItemStack {
            id: uuid::Uuid::new_v4(),
            container_id: uuid::Uuid::new_v4(),
            item_kind: "minecraft:diamond_sword".to_owned(),
            slot: 3,
            components: json!({"minecraft:enchantments": 12345}),
            quantity: 1,
            components_digest: String::new(),
        }
    }

    #[test]
    fn components_round_trip_through_bincode() {
        let item_stack = item_stack();

        let encoded = bincode::serialize(&item_stack).unwrap();
        let decoded: ItemStack = bincode::deserialize(&encoded).unwrap();

        assert_eq!(decoded.components, item_stack.components);
    }

    #[test]
    fn components_remain_structured_in_json() {
        let encoded = serde_json::to_value(item_stack()).unwrap();

        assert_eq!(
            encoded["components"],
            json!({"minecraft:enchantments": 12345})
        );
    }
}
