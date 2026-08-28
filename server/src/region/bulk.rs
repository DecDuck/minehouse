use common::rpc::MinehouseError;

use crate::{
    db::{DatabaseHandle, container_region::RegionType},
    mwms::category::categorize,
    region::{
        ContainerRef, ItemRequest, MAX_STACK, Placement, Region, RegionInventory, RegionItem,
        RegionStack, load_inventory,
    },
};

/// Bulk storage. Allocation is driven by item category: each item goes into a
/// container pinned to its category.
pub struct BulkRegion {
    inventory: RegionInventory,
}

impl BulkRegion {
    /// Loads every bulk container across all bulk region rows as one logical region.
    pub async fn load(db: &DatabaseHandle) -> Result<Self, MinehouseError> {
        let containers = db.list_containers_by_region_type(RegionType::Bulk).await?;
        Ok(Self {
            inventory: load_inventory(db, containers).await?,
        })
    }
}

impl Region for BulkRegion {
    fn items(&self) -> Vec<RegionItem> {
        self.inventory.items()
    }

    fn container_refs(&self) -> Vec<ContainerRef> {
        self.inventory.container_refs()
    }

    fn reserve(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        self.inventory.reserve(requests)
    }

    fn allocate(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        let mut out = Vec::new();
        for req in requests {
            let category = categorize(&req.item_kind);
            let mut remaining = req.quantity;
            for c in self
                .inventory
                .containers
                .iter_mut()
                .filter(|c| c.category == Some(category))
            {
                if remaining == 0 {
                    break;
                }
                // Top up existing partial stacks of the same kind first.
                let partial: Vec<usize> = c
                    .slots
                    .iter()
                    .filter(|(_, s)| s.item_kind == req.item_kind && s.quantity < MAX_STACK)
                    .map(|(k, _)| *k)
                    .collect();
                for slot in partial {
                    if remaining == 0 {
                        break;
                    }
                    let stack = c.slots.get_mut(&slot).unwrap();
                    let put = remaining.min(MAX_STACK - stack.quantity);
                    stack.quantity += put;
                    remaining -= put;
                    out.push(Placement {
                        item_kind: req.item_kind.clone(),
                        container_id: c.id,
                        position: c.position,
                        slot,
                        quantity: put,
                    });
                }
                // Then open new stacks in free slots.
                while remaining > 0 {
                    let Some(slot) = c.first_free_slot() else {
                        break;
                    };
                    let put = remaining.min(MAX_STACK);
                    c.slots.insert(
                        slot,
                        RegionStack {
                            item_kind: req.item_kind.clone(),
                            quantity: put,
                        },
                    );
                    remaining -= put;
                    out.push(Placement {
                        item_kind: req.item_kind.clone(),
                        container_id: c.id,
                        position: c.position,
                        slot,
                        quantity: put,
                    });
                }
            }
            if remaining > 0 {
                tracing::warn!(
                    "bulk: no space for {remaining} of {} (category {category:?})",
                    req.item_kind
                );
            }
        }
        out
    }
}
