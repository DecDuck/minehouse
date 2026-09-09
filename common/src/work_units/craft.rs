use azalea_registry::builtin::ItemKind;
use serde::{Deserialize, Serialize};

use crate::{item_stack::ItemStack, vec::Point, work::WorkUnitDone};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftIngredient {
    pub item_kind: ItemKind,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftRecipe {
    pub recipe_id: uuid::Uuid,
    pub output_item_kind: ItemKind,
    pub output_yield: u32,
    pub ingredients: Vec<CraftIngredient>,
    pub crafts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftLocations {
    pub engine: Point,
    pub input_container_id: uuid::Uuid,
    pub input_container: Point,
    pub output_container_id: uuid::Uuid,
    pub output_container: Point,
    pub output_container_capacity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftProgress {
    pub completed_crafts: u32,
    pub input_snapshot: Option<Vec<ItemStack>>,
    pub output_snapshot: Option<Vec<ItemStack>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftWorkUnit {
    pub recipe: CraftRecipe,
    pub locations: CraftLocations,
    pub progress: CraftProgress,
}

impl WorkUnitDone for CraftWorkUnit {
    fn is_done(&self) -> bool {
        self.progress.completed_crafts >= self.recipe.crafts
            && self.progress.output_snapshot.is_some()
    }
}
