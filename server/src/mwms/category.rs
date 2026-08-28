use std::str::FromStr;

use azalea_registry::builtin::ItemKind;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

use crate::mwms::item_profiles::dft::DefaultItemCategoryProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "item_category", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ItemCategory {
    // All rock types, so stone, deepslate, granite, andesite, etc etc. Includes cobbled and mossy variants. Also rock byproducts, like walls, tiles, slabs, etc etc
    Rocks,
    // All wood types, and their byproducts, like sticks, saplings, slabs, etc etc
    Wood,
    // All tools, and enchantment books
    Tools,
    // Materials, including valuables. Diamonds, gold, iron, etc etc, including nuggets. 
    Materials,
    // All food
    Food,
    // All mob drops that haven't been covered by other categories, like gunpowder or string. Mob drops like iron and carrots should go in other categories
    MobDrops,
    // Everything that doesn't have a place
    Other,   
}

pub trait ItemCategoryProfile {
    fn map_item_kind(&self, kind: &ItemKind) -> ItemCategory;
}

#[enum_dispatch(ItemCategoryProfile)]
pub enum ItemCategoryProfiles {
    Default(DefaultItemCategoryProfile),
}

/// Resolves an `item_kind` string (`"minecraft:stone"` or `"stone"`) to its
/// category, falling back to `Other` for anything unrecognised.
pub fn categorize(item_kind: &str) -> ItemCategory {
    match ItemKind::from_str(item_kind) {
        Ok(kind) => DefaultItemCategoryProfile.map_item_kind(&kind),
        Err(_) => ItemCategory::Other,
    }
}