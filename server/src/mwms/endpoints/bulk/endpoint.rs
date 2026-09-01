use std::collections::HashSet;

use dashmap::DashMap;
use sqlx::types::Uuid;
use tokio::sync::RwLock;

use crate::{
    db::{container::Container, container_region::ContainerRegion},
    mwms::{
        category::ItemCategoryProfiles,
        item_profiles::dft::DefaultItemCategoryProfile,
        storage::{StorageEndpoint, StorageEndpointError, StorageEndpointId},
        transfer::{
            document::{TransferDocumentHandle, TransferDocumentId},
            request::TransferRequest,
        },
    },
};

use super::{container::BulkContainer, document::BulkTransferDocument};

pub struct BulkStorage {
    id: StorageEndpointId,
    region: ContainerRegion,
    /// Categorised containers, kept in sync incrementally by re-index jobs.
    containers: RwLock<Vec<BulkContainer>>,
    /// Transfer documents this endpoint has committed to, keyed by document id.
    documents: DashMap<TransferDocumentId, BulkTransferDocument>,
    /// Maps item kinds to categories when slotting newly-seen containers.
    profile: ItemCategoryProfiles,
}

impl BulkStorage {
    /// Free slots per container across the region, keyed by container id.
    async fn empty_slots(&self) -> Vec<(Uuid, u64)> {
        self.containers
            .read()
            .await
            .iter()
            .map(|entry| (entry.container.id, entry.empty_slots()))
            .collect()
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

        let free: u64 = self.empty_slots().await.into_iter().map(|(_, free)| free).sum();
        if Self::required_slots(request) > free {
            return Err(StorageEndpointError::StorageFull(self.id));
        }

        Ok(())
    }

    /// Commits a transfer document to this endpoint's ledger.
    async fn record(&self, document: TransferDocumentHandle) {
        let id = document.lock().await.id;
        self.documents.insert(id, BulkTransferDocument { document });
    }
}

impl StorageEndpoint for BulkStorage {
    fn new(region: ContainerRegion) -> Self {
        Self {
            id: StorageEndpointId::random(),
            region,
            containers: RwLock::new(Vec::new()),
            documents: DashMap::new(),
            profile: ItemCategoryProfiles::Default(DefaultItemCategoryProfile),
        }
    }

    fn id(&self) -> StorageEndpointId {
        self.id
    }

    async fn reindex(&self, containers: Vec<Container>) -> Result<(), StorageEndpointError> {
        let mut current = self.containers.write().await;

        // Deleted containers: drop anything missing from the new snapshot.
        let incoming: HashSet<_> = containers.iter().map(|container| container.id).collect();
        current.retain(|entry| incoming.contains(&entry.container.id));

        // New containers: categorise from their contents. Unchanged ones (same
        // id) keep their existing allocation untouched.
        let known: HashSet<_> = current.iter().map(|entry| entry.container.id).collect();
        for container in containers {
            if known.contains(&container.id) {
                continue;
            }
            let category = container.categorize(&self.profile);
            current.push(BulkContainer { container, category });
        }

        let unallocated = current.iter().filter(|entry| entry.category.is_none()).count();
        tracing::debug!(
            region = %self.region.id,
            containers = current.len(),
            unallocated,
            "bulk endpoint reindexed",
        );
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

        // Build the shared handle up front so both endpoints record the same
        // document rather than divergent copies.
        let handle = TransferDocumentHandle::new(request.clone().accept());

        // The counterparty checks and records its side; bail if it's unhappy.
        other.negotiate_transfer(request, &handle).await?;

        self.record(handle.clone()).await;
        Ok(handle)
    }

    async fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocumentHandle,
    ) -> Result<(), StorageEndpointError> {
        self.ensure_space(request).await?;
        // Clone the handle, not the document: both endpoints share one Arc.
        self.record(document.clone()).await;
        Ok(())
    }
}
