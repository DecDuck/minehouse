use common::rpc::MinehouseError;
use uuid::Uuid;

use crate::{
    db::DatabaseHandle,
    region::{
        ContainerRef, ItemRequest, Placement, Region, RegionInventory, RegionItem, load_inventory,
    },
};

/// Putaway staging. Everything here should be pushed out to bulk; it never
/// receives items from the planner.
pub struct PutawayRegion {
    inventory: RegionInventory,
}

impl PutawayRegion {
    pub async fn load(db: &DatabaseHandle, region_id: Uuid) -> Result<Self, MinehouseError> {
        let containers = db.list_containers_in_region(region_id).await?;
        Ok(Self {
            inventory: load_inventory(db, containers).await?,
        })
    }
}

impl Region for PutawayRegion {
    fn items(&self) -> Vec<RegionItem> {
        self.inventory.items()
    }

    fn container_refs(&self) -> Vec<ContainerRef> {
        self.inventory.container_refs()
    }

    fn surplus(&self) -> Vec<ItemRequest> {
        self.inventory.as_requests()
    }

    fn reserve(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        self.inventory.reserve(requests)
    }
}
