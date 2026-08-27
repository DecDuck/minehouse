use crate::vec::Point;

/// Models the moving of item stacks between two containers
pub struct Transfer {
    pub from: Point,
    pub to: Point,
    pub slots: Vec<usize>,
}
