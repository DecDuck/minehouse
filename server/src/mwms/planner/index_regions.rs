use std::time::{Duration, Instant};

use common::{
    work::{WorkUnit, WorkUnitData},
    work_units::index_region::IndexRegionWorkUnit,
};
use futures::{StreamExt as _, stream::FuturesUnordered};
use tracing::warn;

use crate::mwms::planner::Planner;

impl Planner {
    pub async fn index_regions(&self) -> Result<(), anyhow::Error> {
        let regions = self.state.db.fetch_all_regions().await?;
        self.sync_storage_endpoints(&regions);

        // Reindex jobs
        let mut promises = FuturesUnordered::new();

        for region in regions {
            let region_id = region.id;
            let wu = IndexRegionWorkUnit {
                region_range: region.world_region.into(),
                output_chunks_scanned: Vec::new(),
                output_containers: Vec::new(),
            };
            let wu: WorkUnit = wu.into();
            let wu_id = wu.id;
            self.state.pool.queue_work_unit(wu, None);
            promises.push(async move {
                (
                    self.state
                        .pool
                        .wait_work_unit(&wu_id, Instant::now() + Duration::from_hours(2))
                        .await,
                    region_id,
                )
            });
        }

        while let Some((result, region_id)) = promises.next().await {
            let result = match result {
                Ok(v) => v,
                Err(err) => {
                    warn!("failed to index region: {err:?}");
                    continue;
                }
            };

            let data = match result.data {
                WorkUnitData::IndexRegion(index_chunk_work_unit) => index_chunk_work_unit,
                _ => unreachable!(),
            };

            self.state
                .db
                .sync_region_containers(region_id, &data.output_containers)
                .await?;
        }

        self.refresh_endpoint_contents().await?;
        Ok(())
    }
}
