use serde::{Deserialize, Serialize};

use crate::{
    ids::WorkUnitId,
    vec::{Point, PointRegion},
    work::{WorkUnit, WorkUnitData::IndexRegion, WorkUnitDone},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexRegionWorkUnit {
    pub region_range: PointRegion,

    pub output_chunks_scanned: usize,
    pub output_containers: Vec<IndexChunkWorkUnitContainer>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexChunkWorkUnitContainer {
    pub point: Point,
    pub capacity: usize,
}

impl WorkUnitDone for IndexRegionWorkUnit {
    fn is_done(&self) -> bool {
        self.region_range.covered_chunks().len() == self.output_chunks_scanned
    }
}