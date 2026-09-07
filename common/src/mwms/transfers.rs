use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::vec::Point;

/// Moves `quantity` items between concrete slots in a container pair.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMove {
    pub from_slot: usize,
    pub to_slot: usize,
    pub quantity: u32,
}

/// Models the movement of item stacks between two containers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Transfer {
    pub from_container: Uuid,
    pub from_position: Point,
    pub to_container: Uuid,
    pub to_position: Point,
    pub moves: Vec<ItemMove>,
}
