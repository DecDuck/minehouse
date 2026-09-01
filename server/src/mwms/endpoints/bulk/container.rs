use crate::{db::container::Container, mwms::category::ItemCategory};

/// A bulk container tagged with the category it's allocated to. `category` is
/// `None` while the container is unallocated (i.e. empty).
pub(super) struct BulkContainer {
    pub(super) container: Container,
    pub(super) category: Option<ItemCategory>,
}

impl BulkContainer {
    /// Free slots, i.e. capacity minus the number of occupied slots.
    pub(super) fn empty_slots(&self) -> u64 {
        self.container
            .capacity
            .saturating_sub(self.container.contents.len() as u64)
    }
}
