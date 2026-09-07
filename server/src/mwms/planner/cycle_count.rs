use std::time::{Duration, Instant};

use common::{
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::index_container::IndexContainerWorkUnit,
};
use futures::{StreamExt as _, stream::FuturesUnordered};
use tracing::warn;

use super::Planner;

impl Planner {
    pub async fn cycle_count(&self) -> Result<(), anyhow::Error> {
        let containers = self.state.db.fetch_all_container_locations().await?;
        let mut pending = FuturesUnordered::new();

        for container in containers {
            let work_unit: WorkUnit = IndexContainerWorkUnit {
                position: Point {
                    x: container.position.x1,
                    y: container.position.y1,
                    z: container.position.z1,
                },
                container_id: container.id,
                output: None,
            }
            .into();
            let work_unit_id = work_unit.id;
            self.state.pool.queue_work_unit(work_unit, None);
            pending.push(async move {
                self.state
                    .pool
                    .wait_work_unit(&work_unit_id, Instant::now() + Duration::from_hours(2))
                    .await
            });
        }

        while let Some(result) = pending.next().await {
            let completed = match result {
                Ok(work_unit) => work_unit,
                Err(error) => {
                    warn!(?error, "failed to cycle count container");
                    continue;
                }
            };
            let WorkUnitData::IndexContainer(data) = completed.data else {
                unreachable!()
            };

            self.state
                .db
                .replace_container_item_stacks(data.container_id, &data.output.unwrap())
                .await?;
        }

        self.refresh_endpoint_contents().await?;
        Ok(())
    }
}
