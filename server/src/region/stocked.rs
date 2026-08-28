use std::collections::HashMap;

use common::{rpc::MinehouseError, vec::Point};
use uuid::Uuid;

use crate::{
    db::DatabaseHandle,
    region::{
        ContainerRef, ItemRequest, MAX_STACK, Placement, Region, RegionInventory, RegionItem,
        load_inventory,
    },
};

#[derive(Clone)]
struct SlotTarget {
    container_id: Uuid,
    position: Point,
    slot: usize,
    item_kind: String,
    quantity: u32,
}

/// A region with a desired per-slot layout (user or order pickface). Demand is the
/// deficit versus the target; allocation fills the target slots. This is where the
/// stack allocation is handled internally.
pub struct StockedRegion {
    inventory: RegionInventory,
    targets: Vec<SlotTarget>,
}

impl StockedRegion {
    /// User pickface: the target layout is the operator-configured `item_slots`.
    pub async fn user_pickface(
        db: &DatabaseHandle,
        region_id: Uuid,
    ) -> Result<Self, MinehouseError> {
        let containers = db.list_containers_in_region(region_id).await?;
        let inventory = load_inventory(db, containers.clone()).await?;

        let mut targets = Vec::new();
        for c in &containers {
            let Some(config) = db.get_user_pickface(c.id).await? else {
                continue;
            };
            let position = Point {
                x: c.position.x1,
                y: c.position.y1,
                z: c.position.z1,
            };
            for (slot, kind) in config.item_slots.iter().enumerate() {
                if kind.is_empty() {
                    continue;
                }
                targets.push(SlotTarget {
                    container_id: c.id,
                    position,
                    slot,
                    item_kind: kind.clone(),
                    quantity: MAX_STACK,
                });
            }
        }
        Ok(Self { inventory, targets })
    }

    /// Order pickface: the target layout is one stack of each distinct item kind
    /// present in the container's category in bulk.
    pub async fn order_pickface(
        db: &DatabaseHandle,
        region_id: Uuid,
    ) -> Result<Self, MinehouseError> {
        let containers = db.list_containers_in_region(region_id).await?;
        let inventory = load_inventory(db, containers.clone()).await?;

        let mut targets = Vec::new();
        for c in &containers {
            let Some(category) = c.category else {
                tracing::warn!("order pickface container {} has no category", c.id);
                continue;
            };
            let position = Point {
                x: c.position.x1,
                y: c.position.y1,
                z: c.position.z1,
            };
            let kinds = db.distinct_bulk_item_kinds(category).await?;
            for (slot, kind) in kinds.into_iter().take(c.capacity as usize).enumerate() {
                targets.push(SlotTarget {
                    container_id: c.id,
                    position,
                    slot,
                    item_kind: kind,
                    quantity: MAX_STACK,
                });
            }
        }
        Ok(Self { inventory, targets })
    }
}

impl Region for StockedRegion {
    fn items(&self) -> Vec<RegionItem> {
        self.inventory.items()
    }

    fn container_refs(&self) -> Vec<ContainerRef> {
        self.inventory.container_refs()
    }

    fn reserve(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        self.inventory.reserve(requests)
    }

    fn demand(&self) -> Vec<ItemRequest> {
        let mut by_kind: HashMap<String, u32> = HashMap::new();
        for t in &self.targets {
            let current = self.inventory.slot_qty(t.container_id, t.slot, &t.item_kind);
            let deficit = t.quantity.saturating_sub(current);
            if deficit > 0 {
                *by_kind.entry(t.item_kind.clone()).or_default() += deficit;
            }
        }
        by_kind
            .into_iter()
            .map(|(item_kind, quantity)| ItemRequest { item_kind, quantity })
            .collect()
    }

    fn allocate(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        let mut out = Vec::new();
        for req in requests {
            let mut remaining = req.quantity;
            for i in 0..self.targets.len() {
                if remaining == 0 {
                    break;
                }
                let target = self.targets[i].clone();
                if target.item_kind != req.item_kind {
                    continue;
                }
                let current =
                    self.inventory
                        .slot_qty(target.container_id, target.slot, &target.item_kind);
                let deficit = target.quantity.saturating_sub(current);
                if deficit == 0 {
                    continue;
                }
                let put = remaining.min(deficit);
                out.push(Placement {
                    item_kind: req.item_kind.clone(),
                    container_id: target.container_id,
                    position: target.position,
                    slot: target.slot,
                    quantity: put,
                });
                self.inventory
                    .add_to_slot(target.container_id, target.slot, &req.item_kind, put);
                remaining -= put;
            }
        }
        out
    }
}
