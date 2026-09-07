use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    item_stack::ItemStack,
    vec::Point,
    work::{WorkUnitData, WorkUnitDone},
};

/// Indexes a container at a point
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexContainerWorkUnit {
    pub position: Point,
    pub container_id: Uuid,

    pub output: Option<Vec<ItemStack>>,
}

impl WorkUnitDone for IndexContainerWorkUnit {
    fn is_done(&self) -> bool {
        self.output.is_some()
    }
}
