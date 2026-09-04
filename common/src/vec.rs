use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PointRegion {
    pub pos1: Point,
    pub pos2: Point,
}

impl PointRegion {
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let pos1 = Point {
            x: self
                .pos1
                .x
                .min(self.pos2.x)
                .max(other.pos1.x.min(other.pos2.x)),
            y: self
                .pos1
                .y
                .min(self.pos2.y)
                .max(other.pos1.y.min(other.pos2.y)),
            z: self
                .pos1
                .z
                .min(self.pos2.z)
                .max(other.pos1.z.min(other.pos2.z)),
        };
        let pos2 = Point {
            x: self
                .pos1
                .x
                .max(self.pos2.x)
                .min(other.pos1.x.max(other.pos2.x)),
            y: self
                .pos1
                .y
                .max(self.pos2.y)
                .min(other.pos1.y.max(other.pos2.y)),
            z: self
                .pos1
                .z
                .max(self.pos2.z)
                .min(other.pos1.z.max(other.pos2.z)),
        };

        (pos1.x <= pos2.x && pos1.y <= pos2.y && pos1.z <= pos2.z).then_some(Self { pos1, pos2 })
    }

    pub fn covered_chunks(&self) -> Vec<PointRegion> {
        const CHUNK_SIZE: f64 = 16.0;

        let min_x = self.pos1.x.min(self.pos2.x);
        let max_x = self.pos1.x.max(self.pos2.x);
        let min_y = self.pos1.y.min(self.pos2.y);
        let max_y = self.pos1.y.max(self.pos2.y);
        let min_z = self.pos1.z.min(self.pos2.z);
        let max_z = self.pos1.z.max(self.pos2.z);

        let first_chunk_x = (min_x / CHUNK_SIZE).floor() as i64;
        let last_chunk_x = (max_x / CHUNK_SIZE).floor() as i64;
        let first_chunk_z = (min_z / CHUNK_SIZE).floor() as i64;
        let last_chunk_z = (max_z / CHUNK_SIZE).floor() as i64;

        let mut chunks = Vec::new();
        for chunk_x in first_chunk_x..=last_chunk_x {
            for chunk_z in first_chunk_z..=last_chunk_z {
                let chunk_min_x = chunk_x as f64 * CHUNK_SIZE;
                let chunk_min_z = chunk_z as f64 * CHUNK_SIZE;

                chunks.push(PointRegion {
                    pos1: Point {
                        x: chunk_min_x,
                        y: min_y,
                        z: chunk_min_z,
                    },
                    pos2: Point {
                        x: chunk_min_x + CHUNK_SIZE - 1.0,
                        y: max_y,
                        z: chunk_min_z + CHUNK_SIZE - 1.0,
                    },
                });
            }
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::{Point, PointRegion};

    #[test]
    fn covered_chunks_uses_minecraft_chunk_boundaries() {
        let region = PointRegion {
            pos1: Point {
                x: 1.0,
                y: 20.0,
                z: 1.0,
            },
            pos2: Point {
                x: 16.0,
                y: 10.0,
                z: 16.0,
            },
        };

        let chunks = region.covered_chunks();

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].pos1.x, 0.0);
        assert_eq!(chunks[0].pos1.y, 10.0);
        assert_eq!(chunks[0].pos1.z, 0.0);
        assert_eq!(chunks[0].pos2.x, 15.0);
        assert_eq!(chunks[0].pos2.y, 20.0);
        assert_eq!(chunks[0].pos2.z, 15.0);
        assert_eq!(chunks[3].pos1.x, 16.0);
        assert_eq!(chunks[3].pos1.z, 16.0);
    }

    #[test]
    fn covered_chunks_floors_negative_coordinates_and_normalizes_corners() {
        let region = PointRegion {
            pos1: Point {
                x: 16.0,
                y: 0.0,
                z: 16.0,
            },
            pos2: Point {
                x: -1.0,
                y: 1.0,
                z: -1.0,
            },
        };

        let chunks = region.covered_chunks();

        assert_eq!(chunks.len(), 9);
        assert_eq!(chunks[0].pos1.x, -16.0);
        assert_eq!(chunks[0].pos1.z, -16.0);
        assert_eq!(chunks[8].pos1.x, 16.0);
        assert_eq!(chunks[8].pos1.z, 16.0);
    }

    #[test]
    fn intersection_clips_chunk_to_region_bounds() {
        let region = PointRegion {
            pos1: Point {
                x: 3.0,
                y: 5.0,
                z: 4.0,
            },
            pos2: Point {
                x: 18.0,
                y: 10.0,
                z: 20.0,
            },
        };
        let chunk = PointRegion {
            pos1: Point {
                x: 0.0,
                y: 5.0,
                z: 0.0,
            },
            pos2: Point {
                x: 15.0,
                y: 10.0,
                z: 15.0,
            },
        };

        assert_eq!(
            region.intersection(&chunk),
            Some(PointRegion {
                pos1: Point {
                    x: 3.0,
                    y: 5.0,
                    z: 4.0,
                },
                pos2: Point {
                    x: 15.0,
                    y: 10.0,
                    z: 15.0,
                },
            })
        );
    }
}
