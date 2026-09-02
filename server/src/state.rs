use std::sync::Arc;

use crate::{db::DatabaseHandle, mwms::planner::PlannerQueue, work::pool::WorkUnitPool};

#[derive(Clone)]
pub struct MinehouseState {
    pub db: Arc<DatabaseHandle>,
    pub pool: Arc<WorkUnitPool>,
    pub planner_queue: PlannerQueue,
}

impl MinehouseState {
    pub fn new(
        db: Arc<DatabaseHandle>,
        pool: Arc<WorkUnitPool>,
        planner_queue: PlannerQueue,
    ) -> Self {
        Self {
            db,
            pool,
            planner_queue,
        }
    }
}
