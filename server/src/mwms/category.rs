use azalea_registry::builtin::ItemKind;
use enum_dispatch::enum_dispatch;

use crate::mwms::item_profiles::dft::DefaultItemCategoryProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    // All rock types, so stone, deepslate, granite, andesite, etc etc. Includes cobbled and mossy variants. Also rock byproducts, like walls, tiles, slabs, etc etc
    Rocks,
    // All wood types, and their byproducts, like sticks, saplings, slabs, etc etc
    Wood,
    /// All other natural stuff, like grass, dirt, clay, sand, gravel, etc etc
    Nature,
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

#[enum_dispatch]
pub trait ItemCategoryProfile {
    fn map_item_kind(&self, kind: &ItemKind) -> ItemCategory;
}

#[enum_dispatch(ItemCategoryProfile)]
pub enum ItemCategoryProfiles {
    Default(DefaultItemCategoryProfile),
}