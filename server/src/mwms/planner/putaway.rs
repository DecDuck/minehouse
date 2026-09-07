use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use common::{
    item_stack::SKU,
    mwms::transfers::{ItemMove, Transfer},
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::transfer::TransferWorkUnit,
};
use dashmap::DashMap;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    db::{
        container::Container,
        container_region::{ContainerRegion, RegionType},
    },
    mwms::{
        endpoints::{StorageEndpoints, bulk::PlannedDestination},
        planner::Planner,
        storage::{StorageEndpoint, StorageEndpointError},
        transfer::document::{
            TransferDocumentHandle, TransferDocumentJobState, TransferDocumentState,
        },
    },
};

struct PlannedPutaway {
    putaway_id: crate::mwms::storage::StorageEndpointId,
    bulk_id: crate::mwms::storage::StorageEndpointId,
    document: TransferDocumentHandle,
    transfers: Vec<Transfer>,
}

#[derive(Clone, Copy)]
struct SourcePlacement {
    container_id: Uuid,
    position: Point,
    slot: usize,
    remaining: u64,
}

impl Planner {
    pub async fn putaway(&self) -> Result<(), anyhow::Error> {
        let regions = self.state.db.fetch_all_regions().await?;
        self.sync_storage_endpoints(&regions);
        self.refresh_endpoint_contents().await?;

        let endpoint_by_region = self
            .endpoints
            .iter()
            .map(|entry| (entry.region_id(), *entry.key()))
            .collect::<std::collections::HashMap<_, _>>();
        let bulk_regions = ordered_regions(&regions, RegionType::Bulk)
            .into_iter()
            .filter_map(|region| {
                endpoint_by_region
                    .get(&region.id)
                    .map(|endpoint_id| (region, *endpoint_id))
            })
            .collect::<Vec<_>>();
        let bulk_endpoint_ids = bulk_regions
            .iter()
            .map(|(_, endpoint_id)| *endpoint_id)
            .collect::<Vec<_>>();
        let putaway_regions = ordered_regions(&regions, RegionType::Putaway);
        let mut unplanned = 0;

        for putaway_region in putaway_regions {
            let Some(putaway_id) = endpoint_by_region.get(&putaway_region.id) else {
                continue;
            };
            let has_items = {
                let Some(endpoint) = self.endpoints.get(putaway_id) else {
                    continue;
                };
                let StorageEndpoints::Putaway(putaway) = endpoint.value() else {
                    continue;
                };
                putaway.has_items().await
            };
            if !has_items {
                continue;
            }

            match plan_putaway(&self.endpoints, *putaway_id, &bulk_endpoint_ids).await {
                Ok(Some(plan)) => {
                    if let Some((bulk_region, _)) = bulk_regions
                        .iter()
                        .find(|(_, endpoint_id)| *endpoint_id == plan.bulk_id)
                    {
                        info!(
                            putaway_region = %putaway_region.id,
                            bulk_region = %bulk_region.id,
                            bulk_priority = bulk_region.priority,
                            work_units = plan.transfers.len(),
                            "putaway transfer planned",
                        );
                    }
                    self.execute_putaway_plan(plan).await?;
                    self.refresh_endpoint_contents().await?;
                }
                Ok(None) => {
                    unplanned += 1;
                    warn!(
                        putaway_region = %putaway_region.id,
                        "no bulk region has capacity for putaway contents",
                    );
                }
                Err(error) => {
                    return Err(anyhow::anyhow!(
                        "failed to plan putaway region {}: {error:?}",
                        putaway_region.id,
                    ));
                }
            }
        }

        if unplanned > 0 {
            anyhow::bail!("{unplanned} putaway region(s) could not be planned");
        }
        Ok(())
    }

    async fn execute_putaway_plan(&self, plan: PlannedPutaway) -> Result<(), anyhow::Error> {
        let result = async {
            ensure!(
                !plan.transfers.is_empty(),
                "putaway plan contains no transfers"
            );
            for transfer in plan.transfers.iter().cloned() {
                let work_unit: WorkUnit = TransferWorkUnit {
                    transfer,
                    completed: Vec::new(),
                    staged: Vec::new(),
                    from_contents: None,
                    to_contents: None,
                }
                .into();
                let work_unit_id = work_unit.id;
                self.state.pool.queue_work_unit(work_unit, None);

                let completed = self
                    .state
                    .pool
                    .wait_work_unit(&work_unit_id, Instant::now() + Duration::from_hours(2))
                    .await?;
                let WorkUnitData::Transfer(completed) = completed.data else {
                    anyhow::bail!("transfer work unit returned with the wrong data type")
                };
                let from_contents = completed
                    .from_contents
                    .context("completed transfer has no source snapshot")?;
                let to_contents = completed
                    .to_contents
                    .context("completed transfer has no destination snapshot")?;
                self.state
                    .db
                    .replace_container_item_stacks(
                        completed.transfer.from_container,
                        &from_contents,
                    )
                    .await?;
                self.state
                    .db
                    .replace_container_item_stacks(completed.transfer.to_container, &to_contents)
                    .await?;
            }
            Ok(())
        }
        .await;

        let document_id = plan.document.lock().await.id;
        {
            let mut document = plan.document.lock().await;
            if let Err(error) = &result {
                document.header.state = TransferDocumentState::Cancelled {
                    reason: error.to_string(),
                };
            } else {
                for job in &mut document.jobs {
                    job.state = TransferDocumentJobState::Completed;
                }
                document.reconcile();
                document.header.state = TransferDocumentState::Closed;
            }
        }

        if let Some(endpoint) = self.endpoints.get(&plan.bulk_id)
            && let StorageEndpoints::Bulk(bulk) = endpoint.value()
        {
            bulk.remove_document(document_id).await;
        }
        if let Some(endpoint) = self.endpoints.get(&plan.putaway_id)
            && let StorageEndpoints::Putaway(putaway) = endpoint.value()
        {
            putaway.remove_document(document_id);
        }

        result
    }
}

async fn plan_putaway(
    endpoints: &DashMap<crate::mwms::storage::StorageEndpointId, StorageEndpoints>,
    putaway_id: crate::mwms::storage::StorageEndpointId,
    bulk_ids: &[crate::mwms::storage::StorageEndpointId],
) -> Result<Option<PlannedPutaway>, anyhow::Error> {
    let Some(putaway_endpoint) = endpoints.get(&putaway_id) else {
        return Ok(None);
    };
    let StorageEndpoints::Putaway(putaway) = putaway_endpoint.value() else {
        return Err(anyhow::anyhow!("putaway endpoint has the wrong type"));
    };

    for bulk_id in bulk_ids {
        let Some(bulk_endpoint) = endpoints.get(bulk_id) else {
            continue;
        };
        let StorageEndpoints::Bulk(bulk) = bulk_endpoint.value() else {
            continue;
        };
        match putaway.putaway_into(bulk).await {
            Ok(document) => {
                let document_id = document.lock().await.id;
                let lowered = async {
                    let requested = document
                        .lock()
                        .await
                        .lines
                        .iter()
                        .map(|(sku, line)| (*sku, line.quantity))
                        .collect();
                    let destinations = bulk
                        .planned_destinations(document_id)
                        .await
                        .context("bulk endpoint lost its negotiated placement plan")?;
                    lower_transfers(putaway.containers_snapshot().await, destinations, requested)
                }
                .await;
                let transfers = match lowered {
                    Ok(transfers) => transfers,
                    Err(error) => {
                        document.lock().await.header.state = TransferDocumentState::Cancelled {
                            reason: error.to_string(),
                        };
                        bulk.remove_document(document_id).await;
                        putaway.remove_document(document_id);
                        return Err(error);
                    }
                };
                return Ok(Some(PlannedPutaway {
                    putaway_id,
                    bulk_id: *bulk_id,
                    document,
                    transfers,
                }));
            }
            Err(StorageEndpointError::StorageFull(_)) => continue,
            Err(error) => return Err(anyhow::anyhow!("transfer negotiation failed: {error:?}")),
        }
    }
    Ok(None)
}

fn lower_transfers(
    source_containers: Vec<Container>,
    mut destinations: HashMap<SKU, Vec<PlannedDestination>>,
    requested: HashMap<SKU, u64>,
) -> Result<Vec<Transfer>, anyhow::Error> {
    let mut sources: HashMap<SKU, Vec<SourcePlacement>> = HashMap::new();
    for container in source_containers {
        let position = Point {
            x: container.position.x1,
            y: container.position.y1,
            z: container.position.z1,
        };
        for (slot, item) in container.contents {
            if item.quantity <= 0 {
                continue;
            }
            sources
                .entry(item.sku())
                .or_default()
                .push(SourcePlacement {
                    container_id: container.id,
                    position,
                    slot,
                    remaining: item.quantity as u64,
                });
        }
    }
    for placements in sources.values_mut() {
        placements.sort_by_key(|placement| (placement.container_id, placement.slot));
    }
    for placements in destinations.values_mut() {
        placements.sort_by_key(|placement| (placement.container_id, placement.slot));
    }

    let mut grouped: HashMap<(Uuid, Uuid), Transfer> = HashMap::new();
    for (sku, mut quantity) in requested {
        let source = sources
            .get_mut(&sku)
            .context("putaway contents no longer satisfy the transfer document")?;
        let destination = destinations
            .remove(&sku)
            .context("bulk placement plan does not satisfy the transfer document")?;
        let destination_capacity = sku.stack_size().max(1) as u64;
        let mut source_index = 0;
        let mut destination_index = 0;
        let mut destination_remaining = destination_capacity;

        while quantity > 0 {
            while source_index < source.len() && source[source_index].remaining == 0 {
                source_index += 1;
            }
            ensure!(
                source_index < source.len(),
                "putaway contents are short for a transfer line"
            );
            ensure!(
                destination_index < destination.len(),
                "bulk placement plan is short for a transfer line"
            );

            let source_placement = &mut source[source_index];
            let destination_placement = destination[destination_index];
            let moved = quantity
                .min(source_placement.remaining)
                .min(destination_remaining);
            let moved = u32::try_from(moved).context("transfer quantity exceeds u32")?;
            let transfer = grouped
                .entry((
                    source_placement.container_id,
                    destination_placement.container_id,
                ))
                .or_insert_with(|| Transfer {
                    from_container: source_placement.container_id,
                    from_position: source_placement.position,
                    to_container: destination_placement.container_id,
                    to_position: destination_placement.position,
                    moves: Vec::new(),
                });
            transfer.moves.push(ItemMove {
                from_slot: source_placement.slot,
                to_slot: destination_placement.slot,
                quantity: moved,
            });

            let moved = moved as u64;
            source_placement.remaining -= moved;
            destination_remaining -= moved;
            quantity -= moved;
            if destination_remaining == 0 {
                destination_index += 1;
                destination_remaining = destination_capacity;
            }
        }
    }

    let mut transfers = grouped.into_values().collect::<Vec<_>>();
    for transfer in &mut transfers {
        transfer
            .moves
            .sort_by_key(|item_move| (item_move.from_slot, item_move.to_slot));
    }
    transfers.sort_by_key(|transfer| (transfer.from_container, transfer.to_container));
    Ok(transfers)
}

fn ordered_regions(regions: &[ContainerRegion], region_type: RegionType) -> Vec<&ContainerRegion> {
    let mut matching: Vec<_> = regions
        .iter()
        .filter(|region| region.r#type == region_type)
        .collect();
    matching.sort_by_key(|region| (region.priority, region.id));
    matching
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{
        db::{Cube, container::Container},
        mwms::endpoints::{bulk::BulkStorage, putaway::PutawayStorage},
    };
    use common::item_stack::ItemStack;
    use sqlx::types::JsonValue;
    use uuid::Uuid;

    fn region(id: u128, region_type: RegionType, priority: i32) -> ContainerRegion {
        ContainerRegion {
            id: Uuid::from_u128(id),
            r#type: region_type,
            priority,
            world_region: Cube {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 1.0,
                y2: 1.0,
                z2: 1.0,
            },
        }
    }

    fn container(region_id: Uuid, occupied: bool) -> Container {
        let id = Uuid::new_v4();
        let contents = if occupied {
            HashMap::from([(0, stone(id))])
        } else {
            HashMap::new()
        };
        Container {
            id,
            region_id,
            position: Cube {
                x1: 0.0,
                y1: 0.0,
                z1: 0.0,
                x2: 1.0,
                y2: 1.0,
                z2: 1.0,
            },
            capacity: 1,
            contents,
        }
    }

    fn stone(container_id: Uuid) -> ItemStack {
        ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: "minecraft:stone".to_owned(),
            slot: 0,
            components: JsonValue::Null,
            quantity: 1,
            components_digest: String::new(),
        }
    }

    #[test]
    fn bulk_regions_are_ordered_by_priority_then_id() {
        let regions = vec![
            region(3, RegionType::Bulk, 20),
            region(2, RegionType::Putaway, 0),
            region(4, RegionType::Bulk, 10),
            region(1, RegionType::Bulk, 10),
        ];

        let ordered = ordered_regions(&regions, RegionType::Bulk)
            .into_iter()
            .map(|region| region.id)
            .collect::<Vec<_>>();

        assert_eq!(
            ordered,
            vec![Uuid::from_u128(1), Uuid::from_u128(4), Uuid::from_u128(3)]
        );
    }

    #[tokio::test]
    async fn putaway_uses_next_bulk_when_first_is_full() {
        let putaway_region = region(1, RegionType::Putaway, 0);
        let putaway = PutawayStorage::new(putaway_region.clone());
        let source = container(putaway_region.id, true);
        let source_id = source.id;
        putaway.reindex(vec![source]).await.unwrap();

        let first_region = region(2, RegionType::Bulk, 0);
        let first = BulkStorage::new(first_region.clone());
        first
            .reindex(vec![container(first_region.id, true)])
            .await
            .unwrap();

        let second_region = region(3, RegionType::Bulk, 1);
        let second = BulkStorage::new(second_region.clone());
        let destination = container(second_region.id, false);
        let destination_id = destination.id;
        second.reindex(vec![destination]).await.unwrap();

        let putaway_id = putaway.id();
        let first_id = first.id();
        let second_id = second.id();
        let endpoints = DashMap::new();
        endpoints.insert(putaway_id, StorageEndpoints::Putaway(putaway));
        endpoints.insert(first_id, StorageEndpoints::Bulk(first));
        endpoints.insert(second_id, StorageEndpoints::Bulk(second));

        let selected = plan_putaway(&endpoints, putaway_id, &[first_id, second_id])
            .await
            .unwrap()
            .unwrap();

        assert_eq!(selected.bulk_id, second_id);
        assert_eq!(selected.transfers.len(), 1);
        let transfer = &selected.transfers[0];
        assert_eq!(transfer.from_container, source_id);
        assert_eq!(transfer.to_container, destination_id);
        assert_eq!(transfer.moves.len(), 1);
        assert_eq!(transfer.moves[0].from_slot, 0);
        assert_eq!(transfer.moves[0].to_slot, 0);
        assert_eq!(transfer.moves[0].quantity, 1);
    }
}
