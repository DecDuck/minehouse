pub mod queue;
pub mod request;

use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use tokio::time::{self, MissedTickBehavior};

use crate::{
    mwms::{endpoints::StorageEndpoints, storage::StorageEndpointId},
    state::MinehouseState,
};

pub use queue::InspectableQueue;
pub use request::PlannerRequest;

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

    /// Drives the planner tick loop, ticking every [`TICK_INTERVAL`].
    ///
    /// The returned future runs forever; spawn it as a background task.
    pub async fn run(self) {
        let mut interval = time::interval(TICK_INTERVAL);
        // Don't fire a burst of catch-up ticks if a tick runs long.
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            self.tick();
        }
    }

    /// Runs a single planning pass: drains the request queue and acts on each request.
    fn tick(&self) {
        while let Some(request) = self.queue.pop() {
            self.handle_request(request);
        }
    }

    fn handle_request(&self, request: PlannerRequest) {
        match request {
            
        }
    }
}
