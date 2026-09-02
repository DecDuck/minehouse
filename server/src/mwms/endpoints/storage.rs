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
        panic!("can't create a generic region")
    }

    fn id(&self) -> StorageEndpointId {
        match self {
            Self::Bulk(storage) => storage.id(),
            Self::Putaway(storage) => storage.id(),
        }
    }

    fn region_id(&self) -> uuid::Uuid {
        match self {
            StorageEndpoints::Bulk(bulk_storage) => bulk_storage.region_id(),
            StorageEndpoints::Putaway(putaway_storage) => putaway_storage.region_id(),
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
