use std::collections::{BTreeMap, HashMap, VecDeque};

use common::{
    mwms::transfers::{ItemMove, Transfer},
    rpc::MinehouseError,
    vec::Point,
};
use uuid::Uuid;

use crate::{
    db::{DatabaseHandle, container::Container},
    mwms::category::ItemCategory,
};

pub mod bulk;
pub mod putaway;
pub mod stocked;

pub use bulk::BulkRegion;
pub use putaway::PutawayRegion;
pub use stocked::StockedRegion;

pub(crate) const MAX_STACK: u32 = 64;

/// A quantity of an item the caller wants moved in or out of a region.
#[derive(Clone, Debug)]
pub struct ItemRequest {
    pub item_kind: String,
    pub quantity: u32,
}

/// A concrete slot within a region that a quantity is taken from or placed into.
#[derive(Clone, Debug)]
pub struct Placement {
    pub item_kind: String,
    pub container_id: Uuid,
    pub position: Point,
    pub slot: usize,
    pub quantity: u32,
}

/// A flattened item entry within a region.
#[derive(Clone, Debug)]
pub struct RegionItem {
    pub item_kind: String,
    pub container_id: Uuid,
    pub slot: usize,
    pub quantity: u32,
}

/// A container's identity and location, used to schedule indexing.
#[derive(Clone, Debug)]
pub struct ContainerRef {
    pub container_id: Uuid,
    pub position: Point,
}

/// A warehouse region abstracted as a single unit. Callers see the flattened item
/// list and move items in and out; each implementation manages its own internal
/// layout (which container and slot items live in).
pub trait Region {
    /// Every item in the region, flattened across its containers.
    fn items(&self) -> Vec<RegionItem>;

    /// Identity and location of each container, for scheduling indexing.
    fn container_refs(&self) -> Vec<ContainerRef>;

    /// Items the region wants pushed out to other regions.
    fn surplus(&self) -> Vec<ItemRequest> {
        Vec::new()
    }

    /// Items the region wants pulled in to reach its target layout.
    fn demand(&self) -> Vec<ItemRequest> {
        Vec::new()
    }

    /// Reserve source slots that together provide up to the requested quantities.
    fn reserve(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        let _ = requests;
        Vec::new()
    }

    /// Allocate destination slots for the requested quantities, managing layout.
    fn allocate(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        let _ = requests;
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct RegionStack {
    pub item_kind: String,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct RegionContainer {
    pub id: Uuid,
    pub position: Point,
    pub capacity: i32,
    pub category: Option<ItemCategory>,
    pub slots: BTreeMap<usize, RegionStack>,
}

impl RegionContainer {
    fn first_free_slot(&self) -> Option<usize> {
        (0..self.capacity as usize).find(|slot| !self.slots.contains_key(slot))
    }
}

/// In-memory model of a region's containers, shared by all region implementations.
#[derive(Clone, Debug, Default)]
pub struct RegionInventory {
    pub containers: Vec<RegionContainer>,
}

impl RegionInventory {
    pub fn items(&self) -> Vec<RegionItem> {
        let mut items = Vec::new();
        for c in &self.containers {
            for (slot, stack) in &c.slots {
                items.push(RegionItem {
                    item_kind: stack.item_kind.clone(),
                    container_id: c.id,
                    slot: *slot,
                    quantity: stack.quantity,
                });
            }
        }
        items
    }

    pub fn container_refs(&self) -> Vec<ContainerRef> {
        self.containers
            .iter()
            .map(|c| ContainerRef {
                container_id: c.id,
                position: c.position,
            })
            .collect()
    }

    /// Aggregate of every item in the region as move requests.
    pub fn as_requests(&self) -> Vec<ItemRequest> {
        let mut by_kind: HashMap<String, u32> = HashMap::new();
        for c in &self.containers {
            for stack in c.slots.values() {
                *by_kind.entry(stack.item_kind.clone()).or_default() += stack.quantity;
            }
        }
        by_kind
            .into_iter()
            .map(|(item_kind, quantity)| ItemRequest { item_kind, quantity })
            .collect()
    }

    fn slot_qty(&self, container_id: Uuid, slot: usize, item_kind: &str) -> u32 {
        self.containers
            .iter()
            .find(|c| c.id == container_id)
            .and_then(|c| c.slots.get(&slot))
            .filter(|s| s.item_kind == item_kind)
            .map(|s| s.quantity)
            .unwrap_or(0)
    }

    fn add_to_slot(&mut self, container_id: Uuid, slot: usize, item_kind: &str, quantity: u32) {
        if let Some(c) = self.containers.iter_mut().find(|c| c.id == container_id) {
            let stack = c.slots.entry(slot).or_insert_with(|| RegionStack {
                item_kind: item_kind.to_string(),
                quantity: 0,
            });
            stack.quantity += quantity;
        }
    }

    /// Takes up to the requested quantities from any containers holding the items.
    pub fn reserve(&mut self, requests: &[ItemRequest]) -> Vec<Placement> {
        let mut out = Vec::new();
        for req in requests {
            let mut remaining = req.quantity;
            for c in &mut self.containers {
                if remaining == 0 {
                    break;
                }
                let matching: Vec<usize> = c
                    .slots
                    .iter()
                    .filter(|(_, s)| s.item_kind == req.item_kind)
                    .map(|(k, _)| *k)
                    .collect();
                for slot in matching {
                    if remaining == 0 {
                        break;
                    }
                    let stack = c.slots.get_mut(&slot).unwrap();
                    let take = remaining.min(stack.quantity);
                    if take == 0 {
                        continue;
                    }
                    out.push(Placement {
                        item_kind: req.item_kind.clone(),
                        container_id: c.id,
                        position: c.position,
                        slot,
                        quantity: take,
                    });
                    stack.quantity -= take;
                    remaining -= take;
                    if stack.quantity == 0 {
                        c.slots.remove(&slot);
                    }
                }
            }
        }
        out
    }
}

/// Loads the in-memory inventory for a set of containers from the database.
pub async fn load_inventory(
    db: &DatabaseHandle,
    containers: Vec<Container>,
) -> Result<RegionInventory, MinehouseError> {
    let mut region_containers = Vec::new();
    for c in containers {
        let contents = db.get_container_contents(c.id).await?;
        let mut slots = BTreeMap::new();
        for s in contents {
            slots.insert(
                s.slot as usize,
                RegionStack {
                    item_kind: s.item_kind,
                    quantity: s.quantity as u32,
                },
            );
        }
        region_containers.push(RegionContainer {
            id: c.id,
            position: Point {
                x: c.position.x1,
                y: c.position.y1,
                z: c.position.z1,
            },
            capacity: c.capacity,
            category: c.category,
            slots,
        });
    }
    Ok(RegionInventory {
        containers: region_containers,
    })
}

/// Plans the concrete transfers to move `requests` from `source` into `dest`,
/// letting each region decide its own internal source and destination slots.
pub fn plan_move(
    source: &mut dyn Region,
    dest: &mut dyn Region,
    requests: &[ItemRequest],
) -> Vec<Transfer> {
    if requests.is_empty() {
        return Vec::new();
    }
    let taken = source.reserve(requests);
    if taken.is_empty() {
        return Vec::new();
    }
    let available = aggregate(&taken);
    let placed = dest.allocate(&available);
    pair(taken, placed)
}

fn aggregate(placements: &[Placement]) -> Vec<ItemRequest> {
    let mut by_kind: HashMap<String, u32> = HashMap::new();
    for p in placements {
        *by_kind.entry(p.item_kind.clone()).or_default() += p.quantity;
    }
    by_kind
        .into_iter()
        .map(|(item_kind, quantity)| ItemRequest { item_kind, quantity })
        .collect()
}

/// Pairs source and destination placements per item kind into transfers grouped
/// by container pair.
fn pair(taken: Vec<Placement>, placed: Vec<Placement>) -> Vec<Transfer> {
    let mut sources: HashMap<String, VecDeque<Placement>> = HashMap::new();
    for p in taken {
        sources.entry(p.item_kind.clone()).or_default().push_back(p);
    }
    let mut dests: HashMap<String, VecDeque<Placement>> = HashMap::new();
    for p in placed {
        dests.entry(p.item_kind.clone()).or_default().push_back(p);
    }

    let mut groups: HashMap<(Uuid, Uuid), (Point, Point, Vec<ItemMove>)> = HashMap::new();
    for (kind, mut srcs) in sources {
        let Some(mut dsts) = dests.remove(&kind) else {
            continue;
        };
        while let (Some(src), Some(dst)) = (srcs.front().cloned(), dsts.front().cloned()) {
            let qty = src.quantity.min(dst.quantity);
            if qty == 0 {
                break;
            }
            let entry = groups
                .entry((src.container_id, dst.container_id))
                .or_insert_with(|| (src.position, dst.position, Vec::new()));
            entry.2.push(ItemMove {
                from_slot: src.slot,
                to_slot: dst.slot,
                quantity: qty,
            });
            srcs.front_mut().unwrap().quantity -= qty;
            dsts.front_mut().unwrap().quantity -= qty;
            if srcs.front().unwrap().quantity == 0 {
                srcs.pop_front();
            }
            if dsts.front().unwrap().quantity == 0 {
                dsts.pop_front();
            }
        }
    }

    groups
        .into_iter()
        .map(
            |((from_container, to_container), (from_position, to_position, moves))| Transfer {
                from_container,
                from_position,
                to_container,
                to_position,
                moves,
            },
        )
        .collect()
}
