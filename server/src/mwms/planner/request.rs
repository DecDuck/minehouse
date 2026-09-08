use oasgen::OaSchema;
use serde::Deserialize;

use crate::mwms::crafting::CraftPlanRequest;

/// A request submitted to the [`Planner`](super::Planner) from any task.
///
/// Requests are pushed onto the planner's [`PlannerQueue`](super::PlannerQueue) and
/// drained on each tick.
#[derive(Debug, Clone, Deserialize, OaSchema)]
pub enum PlannerRequest {
    IndexRegions,
    CycleCount,
    Putaway,
    Craft(CraftPlanRequest),
}
