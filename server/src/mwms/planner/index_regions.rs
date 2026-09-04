use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use common::{
    work::{WorkUnit, WorkUnitData},
    work_units::index_region::IndexRegionWorkUnit,
};
use futures::{StreamExt as _, stream::FuturesUnordered};
use tracing::warn;

use crate::{
    db::container_region::RegionType,
    mwms::{endpoints::StorageEndpoints, planner::Planner, storage::StorageEndpoint},
};

impl Planner {
    pub async fn index_regions(&self) -> Result<(), anyhow::Error> {
        let regions = self.state.db.fetch_all_regions().await?;
        let regions = regions
            .into_iter()
            .map(|v| (v.id, v))
            .collect::<HashMap<_, _>>();
        let region_id_to_endpoint_id = self
            .endpoints
            .iter()
            .map(|v| (v.value().region_id(), *v.key()))
            .collect::<HashMap<_, _>>();

        // Remove unused endpoints
        let unused = region_id_to_endpoint_id
            .iter()
            .filter(|v| !regions.contains_key(v.0))
            .map(|v| v.1);
        for unused in unused {
            self.endpoints.remove(unused);
        }

        // Create new endpoints
        let missing = regions
            .iter()
            .filter(|v| !region_id_to_endpoint_id.contains_key(v.0));
        for (_, container_region) in missing {
            let region = match container_region.r#type {
                RegionType::Bulk => StorageEndpoints::bulk(container_region.clone()),
                RegionType::Pickface => todo!(),
                RegionType::Putaway => StorageEndpoints::putaway(container_region.clone()),
                RegionType::Processing => todo!(),
                RegionType::Order => todo!(),
            };
            self.endpoints.insert(region.id(), region);
        }

        // Reindex jobs
        let mut promises = FuturesUnordered::new();

        for (region_id, region) in regions {
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

        Ok(())
    }
}
