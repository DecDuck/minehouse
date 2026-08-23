use std::sync::Arc;

use crate::db::DatabaseHandle;

#[derive(Clone)]
pub struct MinehouseState {
    pub db: Arc<DatabaseHandle>,
}

impl MinehouseState {
    pub fn new(db: Arc<DatabaseHandle>) -> Self {
        Self { db }
    }
}
