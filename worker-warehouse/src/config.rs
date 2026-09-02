use common::vec::Point;
use serde::Deserialize;
use worker_core::config::WorkerConfig;

#[derive(Deserialize)]
pub struct WarehouseWorkerConfig {
    pub worker: WorkerConfig,
    pub home_square: Point,
}
