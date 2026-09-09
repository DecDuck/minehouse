use std::sync::Arc;

use crate::{
    db::DatabaseHandle,
    mwms::planner::{CraftPlannerWake, PlannerQueue},
    work::pool::WorkUnitPool,
};

#[derive(Clone)]
pub struct MinehouseState {
    pub db: Arc<DatabaseHandle>,
    pub pool: Arc<WorkUnitPool>,
    pub planner_queue: PlannerQueue,
    pub craft_planner_wake: CraftPlannerWake,
}

impl MinehouseState {
    pub fn new(
        db: Arc<DatabaseHandle>,
        pool: Arc<WorkUnitPool>,
        planner_queue: PlannerQueue,
        craft_planner_wake: CraftPlannerWake,
    ) -> Self {
        Self {
            db,
            pool,
            planner_queue,
            craft_planner_wake,
        }
    }
}
