use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use sqlx::types::Uuid;
use tokio::sync::RwLock;

use crate::{
    db::{Cube, container::Container, container_region::ContainerRegion},
    mwms::{
        category::{ItemCategory, ItemCategoryProfile, ItemCategoryProfiles},
        item_profiles::dft::DefaultItemCategoryProfile,
        storage::{StorageEndpoint, StorageEndpointError, StorageEndpointId},
        transfer::{
            document::{TransferDocumentHandle, TransferDocumentId, TransferDocumentState},
            request::TransferRequest,
        },
    },
};

use super::{
    container::BulkContainer,
    document::{BulkTransferDocument, PlacementPlan, SlotRef},
};

/// A container and the slots within it available for a category, plus whether
/// the container is newly allocated (not yet assigned the category).
struct ContainerSlots {
    container_id: Uuid,
    slots: Vec<usize>,
    newly_allocated: bool,
}

pub struct BulkStorage {
    id: StorageEndpointId,
    region: ContainerRegion,
    /// Categorised containers, kept in sync incrementally by re-index jobs.
    containers: RwLock<Vec<BulkContainer>>,
    /// Transfer documents this endpoint has committed to, keyed by document id.
    documents: DashMap<TransferDocumentId, BulkTransferDocument>,
    /// Maps item kinds to categories when slotting containers.
    profile: ItemCategoryProfiles,
}

impl BulkStorage {
    /// Finds slots available for `category`: containers already holding that
    /// category first, then unassigned containers ordered most-central first (the
    /// one physically closest to the most other containers). Newly-allocated
    /// containers keep their `None` category until a placement commits.
    fn find_slots(&self, category: ItemCategory, containers: &[BulkContainer]) -> Vec<ContainerSlots> {
        // Containers already holding this category, with room to spare.
        let mut result: Vec<ContainerSlots> = containers
            .iter()
            .filter(|entry| entry.category == Some(category))
            .filter_map(|entry| {
                let slots = entry.available_slots();
                (!slots.is_empty()).then_some(ContainerSlots {
                    container_id: entry.container.id,
                    slots,
                    newly_allocated: false,
                })
            })
            .collect();

        // Unassigned containers follow as allocation/overflow targets, the one
        // physically closest to the most other containers first.
        let mut unassigned: Vec<(f64, &BulkContainer)> = containers
            .iter()
            .filter(|entry| entry.category.is_none() && !entry.available_slots().is_empty())
            .map(|entry| (centrality(entry, &containers), entry))
            .collect();
        unassigned.sort_by(|(a, _), (b, _)| a.total_cmp(b));

        result.extend(unassigned.into_iter().map(|(_, entry)| ContainerSlots {
            container_id: entry.container.id,
            slots: entry.available_slots(),
            newly_allocated: true,
        }));

        result
    }

    /// Reserves a slot for every stack in `request`, choosing category-matching
    /// containers and allocating new ones as needed. Atomic: nothing is reserved
    /// unless the whole request fits, so a failure leaves no partial state.
    async fn plan_transfer(
        &self,
        request: &TransferRequest,
        document: TransferDocumentId,
    ) -> Result<PlacementPlan, StorageEndpointError> {
        if request.header.to != self.id {
            return Err(StorageEndpointError::EndpointMismatch);
        }

        let mut plan = PlacementPlan::new();
        // Tentative reservations/assignments layered over committed state for
        // just this request, so its lines don't double-book each other's slots.
        let mut used: HashMap<Uuid, HashSet<usize>> = HashMap::new();
        let mut assigned: HashMap<Uuid, ItemCategory> = HashMap::new();
        let mut containers = self.containers.write().await;

        for (sku, line) in &request.lines {
            let category = self.profile.map_item_kind(&sku.kind());
            let mut needed = line.quantity.div_ceil(sku.stack_size().max(1) as u64);

            for candidate in self.find_slots(category, &containers) {
                if needed == 0 {
                    break;
                }
                // Don't mix categories: skip a container this request already
                // earmarked for a different one.
                if assigned
                    .get(&candidate.container_id)
                    .is_some_and(|earmarked| *earmarked != category)
                {
                    continue;
                }

                let taken = used.entry(candidate.container_id).or_default();
                for slot in candidate.slots {
                    if needed == 0 {
                        break;
                    }
                    if !taken.insert(slot) {
                        continue;
                    }
                    if candidate.newly_allocated {
                        assigned.insert(candidate.container_id, category);
                    }
                    plan.entry(*sku).or_default().push(SlotRef {
                        container: candidate.container_id,
                        slot,
                    });
                    needed -= 1;
                }
            }

            if needed > 0 {
                return Err(StorageEndpointError::StorageFull(self.id));
            }
        }

        // Everything fits: commit the reservations and category assignments.
        for (container_id, slots) in used {
            if let Some(entry) = find_mut(&mut containers, container_id) {
                for slot in slots {
                    entry.reserved.insert(slot, document);
                }
            }
        }
        for (container_id, category) in assigned {
            if let Some(entry) = find_mut(&mut containers, container_id) {
                if entry.category.is_none() {
                    entry.category = Some(category);
                }
            }
        }

        Ok(plan)
    }

    /// Drops every reservation `document` made, freeing the slots. A container we
    /// fully freed that holds nothing falls back to unallocated.
    async fn release(&self, document: TransferDocumentId) {
        let mut containers = self.containers.write().await;
        for entry in containers.iter_mut() {
            let freed = entry.reserved.values().any(|id| *id == document);
            entry.reserved.retain(|_, id| *id != document);
            if freed && entry.container.contents.is_empty() && entry.reserved.is_empty() {
                entry.category = None;
            }
        }
    }

    /// Rebuilds every container's reserved slots from the stored transfer plans.
    #[allow(dead_code)]
    pub async fn reconcile(&self) {
        let mut containers = self.containers.write().await;
        for entry in containers.iter_mut() {
            entry.reserved.clear();
        }
        for document in self.documents.iter() {
            let id = *document.key();
            if matches!(document.document.lock().await.header.state, TransferDocumentState::Cancelled(_)) {
                continue;
            }
            for slots in document.plan.values() {
                for slot_ref in slots {
                    if let Some(entry) = find_mut(&mut containers, slot_ref.container) {
                        entry.reserved.insert(slot_ref.slot, id);
                    }
                }
            }
        }
    }

    /// Commits a transfer document and its placement plan to the ledger.
    async fn record(&self, document: TransferDocumentHandle, plan: PlacementPlan) {
        let id = document.lock().await.id;
        self.documents
            .insert(id, BulkTransferDocument { document, plan });
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
        // Validate the snapshot before changing endpoint state.
        let mut incoming = HashMap::with_capacity(containers.len());
        for container in containers {
            if container.region_id != self.region.id {
                return Err(StorageEndpointError::ContainerRegionMismatch);
            }
            if incoming.insert(container.id, container).is_some() {
                return Err(StorageEndpointError::DuplicateContainerId);
            }
        }

        // Serialize snapshot application with transfer planning.
        let mut current = self.containers.write().await;

        // Cancel transfers whose planned slots disappeared or became occupied.
        let invalid_documents: Vec<_> = self
            .documents
            .iter()
            .filter_map(|entry| {
                entry.plan.values().any(|slots| {
                    slots.iter().any(|slot_ref| {
                        incoming
                            .get(&slot_ref.container)
                            .is_none_or(|container| {
                                slot_ref.slot >= container.capacity as usize
                                    || container.contents.contains_key(&slot_ref.slot)
                            })
                    })
                })
                .then_some((*entry.key(), entry.document.clone()))
            })
            .collect();
        let cancelled_documents: HashSet<_> = invalid_documents.iter().map(|(id, _)| *id).collect();
        for (_, document) in invalid_documents {
            document.lock().await.header.state = TransferDocumentState::Cancelled(
                "A planned destination slot is no longer available".to_owned(),
            );
        }

        // Remove containers that are no longer present in the snapshot.
        current.retain(|entry| incoming.contains_key(&entry.container.id));

        // Refresh known containers while preserving valid allocation metadata.
        for entry in &mut *current {
            entry.container = incoming.remove(&entry.container.id).expect("retained incoming container");
            if entry.category.is_none() {
                entry.category = entry.container.categorize(&self.profile);
            }
            entry.reserved.retain(|slot, document| {
                self.documents.contains_key(document)
                    && !cancelled_documents.contains(document)
                    && *slot < entry.container.capacity as usize
                    && !entry.container.contents.contains_key(slot)
            });
        }

        // Add containers first seen in this snapshot and categorize their contents.
        for container in incoming.into_values() {
            let category = container.categorize(&self.profile);
            current.push(BulkContainer {
                container,
                category,
                reserved: HashMap::new(),
            });
        }

        // Report the resulting endpoint inventory.
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

        // Build the shared handle up front so both endpoints share one document.
        let handle = TransferDocumentHandle::new(request.clone().accept());
        let document = handle.lock().await.id;

        // Pre-plan placement on our side if we're the destination.
        let plan = if request.header.to == self.id {
            self.plan_transfer(request, document).await?
        } else {
            PlacementPlan::new()
        };

        // The counterparty checks and plans its side; undo our reservations if
        // it refuses.
        if let Err(error) = other.negotiate_transfer(request, &handle).await {
            self.release(document).await;
            return Err(error);
        }

        self.record(handle.clone(), plan).await;
        Ok(handle)
    }

    async fn negotiate_transfer(
        &self,
        request: &TransferRequest,
        document: &TransferDocumentHandle,
    ) -> Result<(), StorageEndpointError> {
        let id = document.lock().await.id;

        // Plan and reserve our side when we're the destination.
        let plan = if request.header.to == self.id {
            self.plan_transfer(request, id).await?
        } else {
            PlacementPlan::new()
        };

        // Clone the handle, not the document: both endpoints share one Arc.
        self.record(document.clone(), plan).await;
        Ok(())
    }
}

/// Total distance from `entry`'s container to every other container; lower means
/// more central to the region.
fn centrality(entry: &BulkContainer, all: &[BulkContainer]) -> f64 {
    all.iter()
        .filter(|other| other.container.id != entry.container.id)
        .map(|other| distance(&entry.container.position, &other.container.position))
        .sum()
}

/// Euclidean distance between the centres of two container bounding boxes.
fn distance(a: &Cube, b: &Cube) -> f64 {
    let centre = |c: &Cube| ((c.x1 + c.x2) / 2.0, (c.y1 + c.y2) / 2.0, (c.z1 + c.z2) / 2.0);
    let (ax, ay, az) = centre(a);
    let (bx, by, bz) = centre(b);
    ((ax - bx).powi(2) + (ay - by).powi(2) + (az - bz).powi(2)).sqrt()
}

fn find_mut(containers: &mut [BulkContainer], id: Uuid) -> Option<&mut BulkContainer> {
    containers.iter_mut().find(|entry| entry.container.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use common::item_stack::ItemStack;
    use sqlx::types::{JsonValue, Uuid};

    use super::*;
    use crate::{
        db::container_region::RegionType,
        mwms::transfer::document::{TransferDocumentState, TransferLineState, TransferOrder},
    };

    fn region() -> ContainerRegion {
        ContainerRegion {
            id: Uuid::new_v4(),
            r#type: RegionType::Bulk,
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

    fn container(region_id: Uuid, capacity: u64, contents: HashMap<usize, ItemStack>) -> Container {
        Container {
            id: Uuid::new_v4(),
            region_id,
            position: Cube {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 1.0,
                y2: 1.0,
                z2: 1.0,
            },
            capacity,
            contents,
        }
    }

    fn stone_stack(container_id: Uuid, slot: i32) -> ItemStack {
        ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: "minecraft:stone".to_owned(),
            slot,
            components: JsonValue::Null,
            quantity: 1,
            components_digest: String::new(),
        }
    }

    fn request(storage_id: StorageEndpointId, sku: common::item_stack::SKU) -> TransferRequest {
        let mut lines = HashMap::new();
        lines.insert(
            sku,
            crate::mwms::transfer::document::TransferLine {
                sku,
                quantity: 1,
                state: TransferLineState::Available,
            },
        );
        TransferRequest {
            header: TransferOrder {
                to: storage_id,
                from: StorageEndpointId::random(),
                state: TransferDocumentState::Draft,
            },
            lines,
        }
    }

    #[tokio::test]
    async fn concurrent_plans_cannot_share_a_slot() {
        let region = region();
        let storage = BulkStorage::new(region.clone());
        storage
            .reindex(vec![container(region.id, 1, HashMap::new())])
            .await
            .unwrap();
        let sku = stone_stack(Uuid::new_v4(), 0).sku();
        let request = request(storage.id(), sku);

        let (first, second) = tokio::join!(
            storage.plan_transfer(&request, TransferDocumentId::random()),
            storage.plan_transfer(&request, TransferDocumentId::random()),
        );

        assert!(first.is_ok() ^ second.is_ok());
        assert!(matches!(
            first.err().or_else(|| second.err()),
            Some(StorageEndpointError::StorageFull(id)) if id == storage.id()
        ));
    }

    #[tokio::test]
    async fn reindex_updates_existing_container_contents() {
        let region = region();
        let storage = BulkStorage::new(region.clone());
        let initial = container(region.id, 1, HashMap::new());
        let id = initial.id;
        storage.reindex(vec![initial]).await.unwrap();

        let mut contents = HashMap::new();
        contents.insert(0, stone_stack(id, 0));
        storage
            .reindex(vec![Container {
                id,
                region_id: region.id,
                position: Cube {
                    x1: 5.0,
                    y1: 5.0,
                    z1: 5.0,
                    x2: 6.0,
                    y2: 6.0,
                    z2: 6.0,
                },
                capacity: 1,
                contents,
            }])
            .await
            .unwrap();

        let containers = storage.containers.read().await;
        assert_eq!(containers[0].container.position.x1, 5.0);
        assert!(containers[0].container.contents.contains_key(&0));
        assert!(containers[0].available_slots().is_empty());
        assert_eq!(containers[0].category, Some(ItemCategory::Rocks));
    }

    #[tokio::test]
    async fn reindex_rejects_wrong_region_and_duplicate_ids() {
        let region = region();
        let storage = BulkStorage::new(region.clone());
        let wrong_region = super::tests::region();
        let wrong = container(wrong_region.id, 1, HashMap::new());
        assert!(matches!(
            storage.reindex(vec![wrong]).await,
            Err(StorageEndpointError::ContainerRegionMismatch)
        ));

        let duplicate = container(region.id, 1, HashMap::new());
        let duplicate_copy = Container { ..duplicate.clone() };
        assert!(matches!(
            storage.reindex(vec![duplicate, duplicate_copy]).await,
            Err(StorageEndpointError::DuplicateContainerId)
        ));
    }

    #[tokio::test]
    async fn removing_a_planned_container_cancels_its_document() {
        let region = region();
        let storage = BulkStorage::new(region.clone());
        let stored = container(region.id, 1, HashMap::new());
        storage.reindex(vec![stored]).await.unwrap();
        let sku = stone_stack(Uuid::new_v4(), 0).sku();
        let request = request(storage.id(), sku);
        let handle = TransferDocumentHandle::new(request.clone().accept());
        let id = handle.lock().await.id;
        let plan = storage.plan_transfer(&request, id).await.unwrap();
        storage.record(handle, plan).await;

        storage.reindex(Vec::new()).await.unwrap();

        let document = storage.documents.get(&id).expect("cancelled document");
        assert!(matches!(
            document.document.lock().await.header.state,
            TransferDocumentState::Cancelled(ref reason)
                if reason == "A planned destination slot is no longer available"
        ));
        assert!(storage.containers.read().await.is_empty());
    }
}
