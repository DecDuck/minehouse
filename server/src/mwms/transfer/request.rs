use std::collections::HashMap;

use common::item_stack::SKU;

use crate::mwms::transfer::document::{
    TransferDocument, TransferDocumentId, TransferDocumentJob, TransferDocumentJobState,
    TransferLine, TransferOrder,
};

#[derive(Clone)]
pub struct TransferRequest {
    pub header: TransferOrder,
    pub lines: HashMap<SKU, TransferLine>,
}

impl TransferRequest {
    pub fn accept(self) -> TransferDocument {
        let mut jobs = Vec::new();

        for line in self.lines.values() {
            // Split each line into stack-sized jobs so bots can run one SKU in parallel.
            let stack_size = line.sku.stack_size() as u64;
            let mut remaining = line.quantity;
            while remaining > 0 {
                let quantity = remaining.min(stack_size);
                jobs.push(TransferDocumentJob {
                    sku: line.sku,
                    quantity,
                    state: TransferDocumentJobState::Available,
                });
                remaining -= quantity;
            }
        }

        let mut document = TransferDocument {
            id: TransferDocumentId::random(),
            header: self.header,
            lines: self.lines,
            jobs,
        };
        document.reconcile();
        document
    }
}
