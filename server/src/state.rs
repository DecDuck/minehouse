use std::sync::Arc;

use crate::{db::DatabaseHandle, planner::PlannerQueue, work::pool::WorkUnitPool};

#[derive(Clone)]
pub struct MinehouseState {
    pub db: Arc<DatabaseHandle>,
    pub pool: Arc<WorkUnitPool>,
    pub planner: Arc<PlannerQueue>,
}

impl MinehouseState {
    pub fn new(
        db: Arc<DatabaseHandle>,
        pool: Arc<WorkUnitPool>,
        planner: Arc<PlannerQueue>,
    ) -> Self {
        Self { db, pool, planner }
    }
}
