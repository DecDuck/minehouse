use serde::{Deserialize, Serialize};

use crate::{item_stack::ItemStack, mwms::transfers::Transfer, work::WorkUnitDone};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferWorkUnit {
    pub transfer: Transfer,
    /// Indices into `transfer.moves` that the worker has applied.
    pub completed: Vec<usize>,
    /// Source contents after the move, captured in-situ while the container was open.
    pub from_contents: Option<Vec<ItemStack>>,
    /// Destination contents after the move, captured in-situ while the container was open.
    pub to_contents: Option<Vec<ItemStack>>,
}

impl WorkUnitDone for TransferWorkUnit {
    fn is_done(&self) -> bool {
        self.completed.len() == self.transfer.moves.len()
            && self.from_contents.is_some()
            && self.to_contents.is_some()
    }
}
