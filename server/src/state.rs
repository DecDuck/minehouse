use std::sync::Arc;

use crate::{db::DatabaseHandle, work::pool::WorkUnitPool};

#[derive(Clone)]
pub struct MinehouseState {
    pub db: Arc<DatabaseHandle>,
    pub pool: Arc<WorkUnitPool>
}

impl MinehouseState {
    pub fn new(db: Arc<DatabaseHandle>, pool: Arc<WorkUnitPool>) -> Self {
        Self { db, pool }
    }
}
