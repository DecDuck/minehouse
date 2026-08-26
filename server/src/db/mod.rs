use common::vec::{Point, PointRegion};
use sqlx::{
    Decode, Encode, PgPool, Postgres, Type,
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgPoolOptions, PgTypeInfo, PgValueRef},
};

use crate::config::MinehouseConfig;

pub mod container;
pub mod container_region;
pub mod crafting_engine;
pub mod generic_crafting_engine;
pub mod item_stack;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cube {
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
    pub x2: f64,
    pub y2: f64,
    pub z2: f64,
}

impl From<PointRegion> for Cube {
    fn from(value: PointRegion) -> Self {
        Self {
            x1: value.pos1.x,
            y1: value.pos1.y,
            z1: value.pos1.z,
            x2: value.pos2.x,
            y2: value.pos2.y,
            z2: value.pos2.z,
        }
    }
}

impl From<Cube> for PointRegion {
    fn from(value: Cube) -> Self {
        Self {
            pos1: Point {
                x: value.x1,
                y: value.y1,
                z: value.z1,
            },
            pos2: Point {
                x: value.x2,
                y: value.y2,
                z: value.z2,
            },
        }
    }
}

impl Type<Postgres> for Cube {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("cube")
    }
}

impl<'r> Decode<'r, Postgres> for Cube {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        const DIMENSION_MASK: u32 = 0x7fff_ffff;
        const POINT_BIT: u32 = 0x8000_0000;

        let bytes = <&[u8] as Decode<Postgres>>::decode(value)?;
        if bytes.len() < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cube value is missing its header",
            )
            .into());
        }

        let header = u32::from_be_bytes(bytes[0..4].try_into()?);
        let dimensions = header & DIMENSION_MASK;
        if dimensions != 3 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected a 3D cube, got {dimensions} dimensions"),
            )
            .into());
        }

        let is_point = header & POINT_BIT != 0;
        let expected_length = if is_point { 28 } else { 52 };
        if bytes.len() != expected_length {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid 3D cube payload length: {}", bytes.len()),
            )
            .into());
        }

        let coordinate = |offset: usize| -> Result<f64, BoxDynError> {
            Ok(f64::from_be_bytes(bytes[offset..offset + 8].try_into()?))
        };
        let x1 = coordinate(4)?;
        let y1 = coordinate(12)?;
        let z1 = coordinate(20)?;

        if is_point {
            return Ok(Self {
                x1,
                y1,
                z1,
                x2: x1,
                y2: y1,
                z2: z1,
            });
        }

        Ok(Self {
            x1,
            y1,
            z1,
            x2: coordinate(28)?,
            y2: coordinate(36)?,
            z2: coordinate(44)?,
        })
    }
}

impl<'q> Encode<'q, Postgres> for Cube {
    fn encode_by_ref(&self, buffer: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buffer.extend_from_slice(&3_u32.to_be_bytes());
        for coordinate in [self.x1, self.y1, self.z1, self.x2, self.y2, self.z2] {
            buffer.extend_from_slice(&coordinate.to_be_bytes());
        }

        Ok(IsNull::No)
    }
}

pub struct DatabaseHandle {
    pub(crate) pool: PgPool,
}

impl DatabaseHandle {
    pub async fn new(config: &MinehouseConfig) -> Result<Self, anyhow::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.db_url)
            .await?;

        Ok(Self { pool })
    }
}
