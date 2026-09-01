use std::collections::HashMap;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::{
    db::{container::Container, container_region::ContainerRegion},
    mwms::{
        storage::{StorageEndpoint, StorageEndpointError, StorageEndpointId},
        transfer::{
            document::{
                TransferDocumentHandle, TransferDocumentId, TransferDocumentState, TransferLine,
                TransferLineState, TransferOrder,
            },
            request::TransferRequest,
        },
    },
};

/// Putaway staging storage. Items are only ever sent out to another endpoint.
pub struct PutawayStorage {
    id: StorageEndpointId,
    region: ContainerRegion,
    containers: RwLock<Vec<Container>>,
    documents: DashMap<TransferDocumentId, TransferDocumentHandle>,
}

impl PutawayStorage {
    /// Creates a transfer for every item currently indexed in this endpoint.
    pub async fn putaway_into<Other>(
        &self,
        storage: &Other,
    ) -> Result<TransferDocumentHandle, StorageEndpointError>
    where
        Other: StorageEndpoint,
    {
        let mut lines = HashMap::new();
        for container in self.containers.read().await.iter() {
            for item in container.contents.values() {
                if item.quantity <= 0 {
                    continue;
                }
                let sku = item.sku();
                let line = lines.entry(sku).or_insert(TransferLine {
                    sku,
                    quantity: 0,
                    state: TransferLineState::Available,
                });
                line.quantity += item.quantity as u64;
            }
        }

        self.request_transfer(
            storage,
            &TransferRequest {
                header: TransferOrder {
                    to: storage.id(),
                    from: self.id,
                    state: TransferDocumentState::Draft,
                },
                lines,
            },
        )
        .await
    }

    async fn record(&self, document: TransferDocumentHandle) {
        let id = document.lock().await.id;
        self.documents.insert(id, document);
    }
}

impl StorageEndpoint for PutawayStorage {
    fn new(region: ContainerRegion) -> Self {
        Self {
            id: StorageEndpointId::random(),
            region,
            containers: RwLock::new(Vec::new()),
            documents: DashMap::new(),
        }
    }

    fn id(&self) -> StorageEndpointId {
        self.id
    }

    async fn reindex(&self, containers: Vec<Container>) -> Result<(), StorageEndpointError> {
        let mut incoming = HashMap::with_capacity(containers.len());
        for container in containers {
            if container.region_id != self.region.id {
                return Err(StorageEndpointError::ContainerRegionMismatch);
            }
            if incoming.insert(container.id, container).is_some() {
                return Err(StorageEndpointError::DuplicateContainerId);
            }
        }
        *self.containers.write().await = incoming.into_values().collect();
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
        if request.header.from != self.id || request.header.to != other.id() {
            return Err(StorageEndpointError::EndpointMismatch);
        }

        let handle = TransferDocumentHandle::new(request.clone().accept());
        other.negotiate_transfer(request, &handle).await?;
        self.record(handle.clone()).await;
        Ok(handle)
    }

    async fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocumentHandle,
    ) -> Result<(), StorageEndpointError> {
        if request.header.from != self.id || request.header.to == self.id {
            return Err(StorageEndpointError::EndpointMismatch);
        }

        self.record(document.clone()).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Cube, container_region::RegionType};
    use common::item_stack::ItemStack;
    use sqlx::types::{JsonValue, Uuid};

    fn region(region_type: RegionType) -> ContainerRegion {
        ContainerRegion {
            id: Uuid::new_v4(),
            r#type: region_type,
            world_region: Cube {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 10.0,
                y2: 10.0,
                z2: 10.0,
            },
        }
    }

    fn item(container_id: Uuid, kind: &str, quantity: i32) -> ItemStack {
        ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: kind.to_owned(),
            slot: 0,
            components: JsonValue::Null,
            quantity,
            components_digest: String::new(),
        }
    }

    #[tokio::test]
    async fn putaway_transfers_all_indexed_items() {
        let putaway_region = region(RegionType::Putaway);
        let putaway = PutawayStorage::new(putaway_region.clone());
        let container_id = Uuid::new_v4();
        let mut contents = HashMap::new();
        contents.insert(0, item(container_id, "minecraft:stone", 3));
        contents.insert(1, item(container_id, "minecraft:stone", 2));
        contents.insert(2, item(container_id, "minecraft:dirt", 0));
        putaway
            .reindex(vec![Container {
                id: container_id,
                region_id: putaway_region.id,
                position: Cube {
                    x1: 0.0,
                    y1: 0.0,
                    z1: 0.0,
                    x2: 1.0,
                    y2: 1.0,
                    z2: 1.0,
                },
                capacity: 3,
                contents,
            }])
            .await
            .unwrap();

        let bulk_region = region(RegionType::Bulk);
        let bulk = super::super::bulk::BulkStorage::new(bulk_region.clone());
        bulk.reindex(vec![Container {
            id: Uuid::new_v4(),
            region_id: bulk_region.id,
            position: Cube {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 1.0,
                y2: 1.0,
                z2: 1.0,
            },
            capacity: 2,
            contents: HashMap::new(),
        }])
        .await
        .unwrap();

        let document = putaway.putaway_into(&bulk).await.unwrap();
        let document = document.lock().await;
        assert_eq!(document.lines.len(), 1);
        assert_eq!(document.lines.values().next().unwrap().quantity, 5);
        assert_eq!(document.header.from, putaway.id());
        assert_eq!(document.header.to, bulk.id());
    }

    #[tokio::test]
    async fn putaway_rejects_inbound_transfers() {
        let region = region(RegionType::Putaway);
        let putaway = PutawayStorage::new(region);
        let request = TransferRequest {
            header: TransferOrder {
                to: putaway.id(),
                from: StorageEndpointId::random(),
                state: TransferDocumentState::Draft,
            },
            lines: HashMap::new(),
        };
        let document = TransferDocumentHandle::new(request.clone().accept());
        assert!(matches!(
            putaway.negotiate_transfer(&request, &document).await,
            Err(StorageEndpointError::EndpointMismatch)
        ));
    }
}
