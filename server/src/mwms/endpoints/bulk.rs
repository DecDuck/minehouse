//! Example [`StorageEndpoint`] for a bulk region.
//!
//! Bulk storage is the warehouse floor: stock is spread across many far-apart,
//! category-based containers where density matters more than pick speed. This
//! example implements the bookkeeping the trait actually cares about, accepting
//! re-index snapshots and rejecting transfers that would overflow the region.
//! The physical, category-based slotting is left as a comment where it hooks in.

use tokio::sync::{Mutex, RwLock};

use crate::{
    db::{container::Container, container_region::ContainerRegion},
    mwms::{
        storage::{StorageEndpoint, StorageEndpointError, StorageEndpointId},
        transfer::{
            document::{TransferDocument, TransferDocumentHandle},
            request::TransferRequest,
        },
    },
};

pub struct BulkStorage {
    id: StorageEndpointId,
    region: ContainerRegion,
    /// Authoritative container list, replaced wholesale by re-index jobs.
    containers: RwLock<Vec<Container>>,
    /// Transfer documents this endpoint has committed to.
    documents: Mutex<Vec<TransferDocumentHandle>>,
}

impl BulkStorage {
    /// Total number of item stacks the region can hold across every container.
    async fn total_capacity(&self) -> u64 {
        self.containers
            .read()
            .await
            .iter()
            .map(|container| container.capacity.max(0) as u64)
            .sum()
    }

    /// Number of stack-sized slots a request needs, matching how
    /// [`TransferRequest::accept`] splits lines into jobs.
    fn required_slots(request: &TransferRequest) -> u64 {
        request
            .lines
            .values()
            .map(|line| {
                let stack_size = (line.sku.stack_size().max(1)) as u64;
                line.quantity.div_ceil(stack_size)
            })
            .sum()
    }

    /// Rejects a transfer this endpoint can't physically accept. Only the
    /// receiving side reserves space; the sender just ships what it holds.
    async fn ensure_space(&self, request: &TransferRequest) -> Result<(), StorageEndpointError> {
        if request.header.to != self.id {
            return Ok(());
        }

        if Self::required_slots(request) > self.total_capacity().await {
            return Err(StorageEndpointError::StorageFull(self.id));
        }

        Ok(())
    }

    /// Commits a transfer document to this endpoint's ledger.
    async fn record(&self, document: TransferDocumentHandle) {
        self.documents.lock().await.push(document);
    }
}

impl StorageEndpoint for BulkStorage {
    fn new(region: ContainerRegion) -> Self {
        Self {
            id: StorageEndpointId::random(),
            region,
            containers: RwLock::new(Vec::new()),
            documents: Mutex::new(Vec::new()),
        }
    }

    fn id(&self) -> StorageEndpointId {
        self.id
    }

    async fn reindex(&self, containers: Vec<Container>) -> Result<(), StorageEndpointError> {
        // A real bulk slotter would re-place items into category-based
        // containers here; the example just adopts the authoritative snapshot.
        tracing::debug!(
            region = %self.region.id,
            containers = containers.len(),
            "bulk endpoint reindexed",
        );
        *self.containers.write().await = containers;
        Ok(())
    }

    async fn request_transfer<Other>(
        &self,
        other: &Other,
        request: &TransferRequest,
    ) -> Result<TransferDocumentHandle, StorageEndpointError>
    where
        Other: StorageEndpoint,
    {
        // The order must route between exactly these two endpoints.
        let endpoints = [request.header.from, request.header.to];
        if !endpoints.contains(&self.id) || !endpoints.contains(&other.id()) {
            return Err(StorageEndpointError::EndpointMismatch);
        }

        // Make sure our own side can honour the transfer before asking theirs.
        self.ensure_space(request).await?;

        let document = request.clone().accept();

        // The counterparty checks and records its side; bail if it's unhappy.
        other.negotiate_transfer(request, &document).await?;

        // Both endpoints are happy: record and hand back the document.
        let handle = TransferDocumentHandle::new(document);
        self.record(handle.clone()).await;
        Ok(handle)
    }

    async fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocument,
    ) -> Result<(), StorageEndpointError> {
        self.ensure_space(request).await?;
        self.record(TransferDocumentHandle::new(document.clone()))
            .await;
        Ok(())
    }
}
