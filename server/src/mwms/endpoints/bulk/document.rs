use crate::mwms::transfer::document::TransferDocumentHandle;

/// A transfer document this bulk endpoint has committed to.
pub(super) struct BulkTransferDocument {
    /// Retained so the transfer can be settled or inspected later.
    #[allow(dead_code)]
    pub(super) document: TransferDocumentHandle,
}
