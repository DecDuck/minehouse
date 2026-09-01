use crate::{
    db::{container::Container, container_region::ContainerRegion},
    mwms::{
        storage::{StorageEndpoint, StorageEndpointError, StorageEndpointId},
        transfer::{document::TransferDocumentHandle, request::TransferRequest},
    },
};

use super::{bulk::BulkStorage, putaway::PutawayStorage};

/// The storage endpoint strategies supported by the warehouse.
pub enum StorageEndpoints {
    Bulk(BulkStorage),
    Putaway(PutawayStorage),
}

impl StorageEndpoints {
    pub fn bulk(region: ContainerRegion) -> Self {
        Self::Bulk(BulkStorage::new(region))
    }

    pub fn putaway(region: ContainerRegion) -> Self {
        Self::Putaway(PutawayStorage::new(region))
    }
}

impl StorageEndpoint for StorageEndpoints {
    fn new(region: ContainerRegion) -> Self {
        Self::bulk(region)
    }

    fn id(&self) -> StorageEndpointId {
        match self {
            Self::Bulk(storage) => storage.id(),
            Self::Putaway(storage) => storage.id(),
        }
    }

    async fn reindex(&self, containers: Vec<Container>) -> Result<(), StorageEndpointError> {
        match self {
            Self::Bulk(storage) => storage.reindex(containers).await,
            Self::Putaway(storage) => storage.reindex(containers).await,
        }
    }

    async fn request_transfer<Other>(
        &self,
        other: &Other,
        request: &TransferRequest,
    ) -> Result<TransferDocumentHandle, StorageEndpointError>
    where
        Other: StorageEndpoint,
    {
        match self {
            Self::Bulk(storage) => storage.request_transfer(other, request).await,
            Self::Putaway(storage) => storage.request_transfer(other, request).await,
        }
    }

    async fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocumentHandle,
    ) -> Result<(), StorageEndpointError> {
        match self {
            Self::Bulk(storage) => storage.negotiate_transfer(request, document).await,
            Self::Putaway(storage) => storage.negotiate_transfer(request, document).await,
        }
    }
}
