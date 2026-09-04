use serde::{Deserialize, Serialize};

use crate::{
    vec::{Point, PointRegion},
    work::WorkUnitDone,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexRegionWorkUnit {
    pub region_range: PointRegion,

    pub output_chunks_scanned: Vec<PointRegion>,
    pub output_containers: Vec<IndexChunkWorkUnitContainer>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexChunkWorkUnitContainer {
    pub point: Point,
    pub capacity: usize,
}

impl WorkUnitDone for IndexRegionWorkUnit {
    fn is_done(&self) -> bool {
        self.region_range
            .covered_chunks()
            .iter()
            .all(|chunk| self.output_chunks_scanned.contains(chunk))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        vec::{Point, PointRegion},
        work::WorkUnitDone,
    };

    use super::IndexRegionWorkUnit;

    fn work_unit() -> IndexRegionWorkUnit {
        IndexRegionWorkUnit {
            region_range: PointRegion {
                pos1: Point {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                pos2: Point {
                    x: 16.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            output_chunks_scanned: Vec::new(),
            output_containers: Vec::new(),
        }
    }

    #[test]
    fn completion_tracks_chunk_identity_instead_of_submission_order() {
        let mut work_unit = work_unit();
        let chunks = work_unit.region_range.covered_chunks();

        work_unit.output_chunks_scanned.push(chunks[1].clone());
        assert!(!work_unit.is_done());

        work_unit.output_chunks_scanned.push(chunks[0].clone());
        assert!(work_unit.is_done());
    }

    #[test]
    fn duplicate_chunk_progress_does_not_complete_work() {
        let mut work_unit = work_unit();
        let chunk = work_unit.region_range.covered_chunks()[0].clone();

        work_unit.output_chunks_scanned.push(chunk.clone());
        work_unit.output_chunks_scanned.push(chunk);

        assert!(!work_unit.is_done());
    }
}
