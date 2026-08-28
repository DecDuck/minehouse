use std::sync::Arc;

use common::{
    ids::WorkUnitId,
    mwms::transfers::Transfer,
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::{index_container::IndexContainerWorkUnit, transfer::TransferWorkUnit},
};
use uuid::Uuid;

use crate::{db::container::Container, region::Region, state::MinehouseState};

pub(crate) fn container_point(container: &Container) -> Point {
    Point {
        x: container.position.x1,
        y: container.position.y1,
        z: container.position.z1,
    }
}

pub(crate) fn enqueue_index(
    state: &Arc<MinehouseState>,
    container_id: Uuid,
    position: Point,
) -> bool {
    let unit = WorkUnit {
        id: WorkUnitId::new(),
        data: WorkUnitData::IndexContainer(IndexContainerWorkUnit {
            position,
            container_id,
            output: None,
        }),
    };
    state
        .pool
        .queue_work_unit(unit, None, Some(format!("index:{container_id}")))
}

/// Schedules an index for every container in a region to refresh its DB snapshot.
pub(crate) fn index_region(state: &Arc<MinehouseState>, region: &dyn Region) {
    for c in region.container_refs() {
        enqueue_index(state, c.container_id, c.position);
    }
}

fn enqueue_transfer(state: &Arc<MinehouseState>, transfer: Transfer, dedup_key: String) -> bool {
    if transfer.moves.is_empty() {
        return false;
    }
    let unit = WorkUnit {
        id: WorkUnitId::new(),
        data: WorkUnitData::Transfer(TransferWorkUnit {
            transfer,
            completed: Vec::new(),
            from_contents: None,
            to_contents: None,
        }),
    };
    state.pool.queue_work_unit(unit, None, Some(dedup_key))
}

/// Enqueues the transfers produced by a region move, deduped per container pair.
pub(crate) fn enqueue_transfers(state: &Arc<MinehouseState>, transfers: Vec<Transfer>) {
    for transfer in transfers {
        let key = format!("transfer:{}:{}", transfer.from_container, transfer.to_container);
        enqueue_transfer(state, transfer, key);
    }
}
