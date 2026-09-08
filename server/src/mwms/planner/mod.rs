pub mod queue;
pub mod request;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use dashmap::DashMap;
use tokio::time::{self, MissedTickBehavior};
use tracing::error;

use crate::{
    db::container_region::{ContainerRegion, RegionType},
    mwms::{
        endpoints::StorageEndpoints,
        storage::{StorageEndpoint, StorageEndpointId},
    },
    state::MinehouseState,
};

pub use queue::InspectableQueue;
pub use request::PlannerRequest;
pub mod craft;
pub mod cycle_count;
pub mod index_regions;
pub mod putaway;

/// Inspectable queue of pending [`PlannerRequest`]s: push from any task, snapshot for a UI.
pub type PlannerQueue = InspectableQueue<PlannerRequest>;

/// How often the planner wakes up to (re)evaluate the work unit pool.
const TICK_INTERVAL: Duration = Duration::from_millis(20);

/// Drives periodic planning: drains queued [`PlannerRequest`]s and schedules work
/// against the shared [`MinehouseState`] (work pool and database).
pub struct Planner {
    state: Arc<MinehouseState>,
    endpoints: DashMap<StorageEndpointId, StorageEndpoints>,
    queue: PlannerQueue,
}

impl Planner {
    pub fn new(state: Arc<MinehouseState>, queue: PlannerQueue) -> Self {
        Self {
            state,
            endpoints: DashMap::new(),
            queue,
        }
    }

    fn sync_storage_endpoints(&self, regions: &[ContainerRegion]) {
        let supported: HashMap<_, _> = regions
            .iter()
            .filter(|region| matches!(region.r#type, RegionType::Bulk | RegionType::Putaway))
            .map(|region| (region.id, region.r#type))
            .collect();
        let obsolete: Vec<_> = self
            .endpoints
            .iter()
            .filter(|entry| supported.get(&entry.region_id()) != Some(&entry.region_type()))
            .map(|entry| *entry.key())
            .collect();
        for endpoint_id in obsolete {
            self.endpoints.remove(&endpoint_id);
        }

        let existing: HashSet<_> = self
            .endpoints
            .iter()
            .map(|entry| entry.region_id())
            .collect();
        for region in regions
            .iter()
            .filter(|region| !existing.contains(&region.id))
        {
            let endpoint = match region.r#type {
                RegionType::Bulk => StorageEndpoints::bulk(region.clone()),
                RegionType::Putaway => StorageEndpoints::putaway(region.clone()),
                RegionType::Pickface | RegionType::Processing | RegionType::Order => continue,
            };
            self.endpoints.insert(endpoint.id(), endpoint);
        }
    }

    async fn refresh_endpoint_contents(&self) -> Result<(), anyhow::Error> {
        let mut containers_by_region: HashMap<_, Vec<_>> = HashMap::new();
        for container in self.state.db.fetch_all_containers().await? {
            containers_by_region
                .entry(container.region_id)
                .or_default()
                .push(container);
        }

        let endpoint_ids: Vec<_> = self.endpoints.iter().map(|entry| *entry.key()).collect();
        for endpoint_id in endpoint_ids {
            let Some(endpoint) = self.endpoints.get(&endpoint_id) else {
                continue;
            };
            let containers = containers_by_region
                .remove(&endpoint.region_id())
                .unwrap_or_default();
            endpoint.reindex(containers).await.map_err(|error| {
                anyhow::anyhow!("failed to reindex storage endpoint: {error:?}")
            })?;
        }
        Ok(())
    }

    /// Drives the planner tick loop, ticking every [`TICK_INTERVAL`].
    ///
    /// The returned future runs forever; spawn it as a background task.
    pub async fn run(self) {
        let mut interval = time::interval(TICK_INTERVAL);
        // Don't fire a burst of catch-up ticks if a tick runs long.
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            self.tick().await;
        }
    }

    /// Runs a single planning pass: drains the request queue and acts on each request.
    async fn tick(&self) {
        let mut waiting = Vec::new();
        while let Some(request) = self.queue.pop() {
            match self.handle_request(request).await {
                Ok(Some(request)) => waiting.push(request),
                Ok(None) => {}
                Err(error) => error!(?error, "planner request failed"),
            }
        }
        for request in waiting {
            self.queue.push(request);
        }
    }

    async fn handle_request(
        &self,
        request: PlannerRequest,
    ) -> Result<Option<PlannerRequest>, anyhow::Error> {
        match request {
            PlannerRequest::IndexRegions => {
                self.index_regions().await?;
            }
            PlannerRequest::CycleCount => {
                self.cycle_count().await?;
            }
            PlannerRequest::Putaway => {
                self.putaway().await?;
            }
            PlannerRequest::Craft(request) => {
                let retry = request.clone();
                let job_id = request
                    .job_id
                    .as_deref()
                    .and_then(|id| uuid::Uuid::parse_str(id).ok());
                match self.craft(request).await {
                    Ok(craft::CraftAttempt::Waiting) => {
                        return Ok(Some(PlannerRequest::Craft(retry)));
                    }
                    Ok(craft::CraftAttempt::Completed) => {}
                    Err(error) => {
                        if let Some(job_id) = job_id {
                            let _ = self
                                .state
                                .db
                                .update_craft_job(
                                    job_id,
                                    crate::mwms::crafting::CraftJobState::Failed,
                                    0,
                                    Some(error.to_string()),
                                )
                                .await;
                        }
                        return Err(error);
                    }
                }
            }
        }
        Ok(None)
    }
}
