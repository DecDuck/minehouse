use azalea::Client;
use common::{
    vec::{Point, PointRegion},
    work::{WorkUnit, WorkUnitData},
};
use worker_core::{client::WorkUnitClient, tick::WorkerControlLoop};

use crate::{
    craft::craft, index_container::index_container, index_region::index_region, transfer::transfer,
};

pub struct WarehouseWorker;

fn axis_distance(value: f64, first: f64, second: f64) -> f64 {
    let min = first.min(second);
    let max = first.max(second);
    if value < min {
        min - value
    } else if value > max {
        value - max
    } else {
        0.0
    }
}

fn point_region_distance_squared(point: Point, region: &PointRegion) -> f64 {
    let dx = axis_distance(point.x, region.pos1.x, region.pos2.x);
    let dy = axis_distance(point.y, region.pos1.y, region.pos2.y);
    let dz = axis_distance(point.z, region.pos1.z, region.pos2.z);
    dx * dx + dy * dy + dz * dz
}

fn work_unit_distance_squared(work_unit: &WorkUnit, point: Point) -> f64 {
    match &work_unit.data {
        WorkUnitData::IndexRegion(data) => point_region_distance_squared(point, &data.region_range),
        WorkUnitData::IndexContainer(data) => {
            let dx = point.x - data.position.x;
            let dy = point.y - data.position.y;
            let dz = point.z - data.position.z;
            dx * dx + dy * dy + dz * dz
        }
        WorkUnitData::Transfer(data) => {
            let position = data.transfer.from_position;
            let dx = point.x - position.x;
            let dy = point.y - position.y;
            let dz = point.z - position.z;
            dx * dx + dy * dy + dz * dz
        }
        WorkUnitData::Craft(data) => {
            let position = data.locations.engine;
            let dx = point.x - position.x;
            let dy = point.y - position.y;
            let dz = point.z - position.z;
            dx * dx + dy * dy + dz * dz
        }
    }
}

fn select_closest_work_unit(work_units: Vec<WorkUnit>, player_position: Point) -> Option<WorkUnit> {
    work_units
        .into_iter()
        .filter(WarehouseWorker::can_accept)
        .min_by(|left, right| {
            work_unit_distance_squared(left, player_position)
                .total_cmp(&work_unit_distance_squared(right, player_position))
        })
}

impl WorkerControlLoop for WarehouseWorker {
    fn can_accept(wu: &WorkUnit) -> bool {
        match wu.data {
            WorkUnitData::IndexRegion(_)
            | WorkUnitData::IndexContainer(_)
            | WorkUnitData::Transfer(_)
            | WorkUnitData::Craft(_) => true,
        }
    }

    fn select_work(work_units: Vec<WorkUnit>, mc_client: &Client) -> Option<WorkUnit> {
        let player_position = mc_client.entity().position().ok()?;
        let player_position = Point {
            x: player_position.x,
            y: player_position.y,
            z: player_position.z,
        };

        select_closest_work_unit(work_units, player_position)
    }

    async fn work(
        wu: WorkUnit,
        client: &WorkUnitClient,
        mc_client: &Client,
    ) -> Result<(), anyhow::Error> {
        match wu.data {
            WorkUnitData::IndexRegion(_) => index_region(wu, client, mc_client).await,
            WorkUnitData::IndexContainer(_) => index_container(wu, client, mc_client).await,
            WorkUnitData::Transfer(_) => transfer(wu, client, mc_client).await,
            WorkUnitData::Craft(_) => craft(wu, client, mc_client).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use common::{
        ids::WorkUnitId,
        mwms::transfers::{ItemMove, Transfer},
        vec::Point,
        work::{WorkUnit, WorkUnitData},
        work_units::{index_container::IndexContainerWorkUnit, transfer::TransferWorkUnit},
    };
    use uuid::Uuid;

    use super::select_closest_work_unit;

    fn container_work_unit(id: WorkUnitId, position: Point) -> WorkUnit {
        WorkUnit {
            id,
            data: WorkUnitData::IndexContainer(IndexContainerWorkUnit {
                position,
                container_id: Uuid::new_v4(),
                output: None,
            }),
        }
    }

    fn transfer_work_unit(id: WorkUnitId, source: Point) -> WorkUnit {
        WorkUnit {
            id,
            data: WorkUnitData::Transfer(TransferWorkUnit {
                transfer: Transfer {
                    from_container: Uuid::new_v4(),
                    from_position: source,
                    to_container: Uuid::new_v4(),
                    to_position: Point {
                        x: 100.0,
                        y: 64.0,
                        z: 100.0,
                    },
                    moves: vec![ItemMove {
                        from_slot: 0,
                        to_slot: 0,
                        quantity: 1,
                    }],
                },
                completed: Vec::new(),
                staged: Vec::new(),
                from_contents: None,
                to_contents: None,
            }),
        }
    }

    #[test]
    fn selects_the_closest_available_work_unit() {
        let near_id = WorkUnitId::new();
        let near = container_work_unit(
            near_id,
            Point {
                x: 2.0,
                y: 64.0,
                z: 1.0,
            },
        );
        let far = container_work_unit(
            WorkUnitId::new(),
            Point {
                x: 20.0,
                y: 64.0,
                z: 20.0,
            },
        );

        let selected = select_closest_work_unit(
            vec![far, near],
            Point {
                x: 0.5,
                y: 64.0,
                z: 0.5,
            },
        )
        .unwrap();

        assert_eq!(selected.id, near_id);
    }

    #[test]
    fn ranks_a_transfer_by_its_source_container() {
        let transfer_id = WorkUnitId::new();
        let transfer = transfer_work_unit(
            transfer_id,
            Point {
                x: 1.0,
                y: 64.0,
                z: 1.0,
            },
        );
        let scan = container_work_unit(
            WorkUnitId::new(),
            Point {
                x: 20.0,
                y: 64.0,
                z: 20.0,
            },
        );

        let selected = select_closest_work_unit(
            vec![scan, transfer],
            Point {
                x: 0.0,
                y: 64.0,
                z: 0.0,
            },
        )
        .unwrap();

        assert_eq!(selected.id, transfer_id);
    }
}
