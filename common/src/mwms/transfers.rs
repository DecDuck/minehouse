use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::vec::Point;

/// Moves `quantity` items from `from_slot` of the source container into `to_slot`
/// of the destination container.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ItemMove {
    pub from_slot: usize,
    pub to_slot: usize,
    pub quantity: u32,
}

/// Models the moving of item stacks between two containers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transfer {
    pub from_container: Uuid,
    pub from_position: Point,
    pub to_container: Uuid,
    pub to_position: Point,
    pub moves: Vec<ItemMove>,
}
