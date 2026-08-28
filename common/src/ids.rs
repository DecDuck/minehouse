use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct WorkUnitId(Uuid);

impl WorkUnitId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
