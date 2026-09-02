use common::vec::Point;
use uuid::Uuid;

/// A request submitted to the [`Planner`](super::Planner) from any task.
///
/// Requests are pushed onto the planner's [`PlannerQueue`](super::PlannerQueue) and
/// drained on each tick.
#[derive(Debug, Clone)]
pub enum PlannerRequest {
    IndexRegions,
}
