use std::collections::HashMap;

use common::item_stack::SKU;
use sqlx::types::Uuid;

use crate::mwms::transfer::document::TransferDocumentHandle;

/// A reserved destination slot within a specific container.
#[derive(Clone, Copy)]
pub(super) struct SlotRef {
    pub(super) container: Uuid,
    pub(super) slot: usize,
}

/// Where a stored transfer intends to place each line, keyed by SKU.
pub(super) type PlacementPlan = HashMap<SKU, Vec<SlotRef>>;

/// A transfer document this bulk endpoint has committed to, plus the slots it
/// reserved for each line.
pub(super) struct BulkTransferDocument {
    /// Retained so the transfer can be settled or inspected later.
    #[allow(dead_code)]
    pub(super) document: TransferDocumentHandle,
    pub(super) plan: PlacementPlan,
}
