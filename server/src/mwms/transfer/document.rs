use std::{collections::HashMap, hash::BuildHasher, ops::Deref, sync::Arc};

use common::item_stack::SKU;
use tokio::sync::Mutex;

use crate::mwms::storage::StorageEndpointId;

#[derive(Clone)]
pub enum TransferDocumentState {
    Draft,
    Released,
    Working,
    Closed,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct TransferDocumentId(u64);

impl TransferDocumentId {
    pub fn random() -> Self {
        Self(std::hash::RandomState::new().hash_one(()))
    }
}

#[derive(Clone)]
pub struct TransferDocument {
    pub id: TransferDocumentId,
    pub header: TransferOrder,
    pub lines: HashMap<SKU, TransferLine>,
    pub jobs: Vec<TransferDocumentJob>,
}

#[derive(Clone)]
pub struct TransferOrder {
    pub to: StorageEndpointId,
    pub from: StorageEndpointId,
    pub state: TransferDocumentState,
}

#[derive(Clone)]
pub enum TransferLineState {
    Available,                // all remaining
    InTransit(u64, u64, u64), // available, in transit, completed
    Completed,                // all moved
}

#[derive(Clone)]
pub struct TransferLine {
    pub sku: SKU,
    pub quantity: u64,
    pub state: TransferLineState,
}

#[derive(Clone)]
pub enum TransferDocumentJobState {
    Available,
    InTransit,
    Completed,
}

#[derive(Clone)]
pub struct TransferDocumentJob {
    pub sku: SKU,
    pub quantity: u64,
    pub state: TransferDocumentJobState,
}

impl TransferDocument {
    /// Fixes all the state in all the transfer lines based on the completed job statuses
    pub fn reconcile(&mut self) {
        for (_, transfer_line) in &mut self.lines {
            let jobs = self
                .jobs
                .iter()
                .filter(|v| v.sku == transfer_line.sku)
                .collect::<Vec<_>>();

            if jobs
                .iter()
                .all(|v| matches!(v.state, TransferDocumentJobState::Available))
            {
                transfer_line.state = TransferLineState::Available;
                continue;
            }

            if jobs
                .iter()
                .all(|v| matches!(v.state, TransferDocumentJobState::Completed))
            {
                transfer_line.state = TransferLineState::Completed;
                continue;
            }

            let (available, in_transit, completed) =
                jobs.iter().fold((0, 0, 0), |mut value, job| {
                    match job.state {
                        TransferDocumentJobState::Available => value.0 += 1,
                        TransferDocumentJobState::InTransit => value.1 += 1,
                        TransferDocumentJobState::Completed => value.2 += 1,
                    }
                    value
                });

            transfer_line.state = TransferLineState::InTransit(available, in_transit, completed);
        }
    }
}

#[derive(Clone)]
pub struct TransferDocumentHandle(Arc<Mutex<TransferDocument>>);

impl TransferDocumentHandle {
    pub fn new(document: TransferDocument) -> Self {
        Self(Arc::new(Mutex::new(document)))
    }

    pub fn inner(&self) -> &Arc<Mutex<TransferDocument>> {
        &self.0
    }
}

impl Deref for TransferDocumentHandle {
    type Target = Arc<Mutex<TransferDocument>>;

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}