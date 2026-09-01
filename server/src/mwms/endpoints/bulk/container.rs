use std::collections::HashMap;

use crate::{
    db::container::Container,
    mwms::{category::ItemCategory, transfer::document::TransferDocumentId},
};

/// A bulk container tagged with the category it's allocated to and the slots
/// reserved by in-flight transfers. `category` is `None` while unallocated.
pub(super) struct BulkContainer {
    pub(super) container: Container,
    pub(super) category: Option<ItemCategory>,
    /// Slot index to the transfer document that reserved it.
    pub(super) reserved: HashMap<usize, TransferDocumentId>,
}

impl BulkContainer {
    /// Slots that are neither occupied by a stack nor reserved by a transfer.
    pub(super) fn available_slots(&self) -> Vec<usize> {
        (0..self.container.capacity as usize)
            .filter(|slot| {
                !self.container.contents.contains_key(slot) && !self.reserved.contains_key(slot)
            })
            .collect()
    }
}
