use std::hash::BuildHasher;

use uuid::Uuid;

use crate::{
    db::{container::Container, container_region::ContainerRegion},
    mwms::transfer::{document::TransferDocumentHandle, request::TransferRequest},
};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct StorageEndpointId(u64);

impl StorageEndpointId {
    pub fn random() -> Self {
        Self(std::hash::RandomState::new().hash_one(()))
    }
}

#[derive(Debug)]
pub enum StorageEndpointError {
    StorageFull(StorageEndpointId),
    /// The transfer order doesn't route between the two negotiating endpoints.
    EndpointMismatch,
    ContainerRegionMismatch,
    DuplicateContainerId,
}

/// Transfers happen between two endpoints, who represent a set of containers
/// with a specific storage strategy
///
/// e.g. bulk storage places things in far away, category-based containers
/// e.g. order_pickface storage places things tight together, for fast picking
/// e.g. user_pickface storage places things tight together, but in category-based containers
pub trait StorageEndpoint {
    /// Creates a new storage endpoitn around a region
    fn new(region: ContainerRegion) -> Self;
    /// Randomly generated ID, static throughout the lifetime of the program
    fn id(&self) -> StorageEndpointId;
    /// Region ID
    fn region_id(&self) -> Uuid;

    /// Passed from a re-index job, this is the new authoriative list of containers
    fn reindex(
        &self,
        containers: Vec<Container>,
    ) -> impl Future<Output = Result<(), StorageEndpointError>>;

    /// Requests a transfer for this storage endpoint. The transfer to and from must match this and other storage endpoint
    /// May fail if there is not enough space in either storage endpoint
    ///
    /// If both storage endpoints are happy with it, they both record the transfer document and return it.
    fn request_transfer<Other>(
        &self,
        other: &Other,
        request: &TransferRequest,
    ) -> impl Future<Output = Result<TransferDocumentHandle, StorageEndpointError>>
    where
        Other: StorageEndpoint;

    /// Called by the storage endpoint from request_transfer, to ensure this storage endpoint is happy with it
    /// The shared document handle is recorded by both endpoints
    fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocumentHandle,
    ) -> impl Future<Output = Result<(), StorageEndpointError>>;
}
