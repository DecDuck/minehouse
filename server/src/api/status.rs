use std::sync::Arc;

use axum::{Json, extract::State};
use common::work::WorkUnitData;
use oasgen::{OaSchema, oasgen};
use serde::Serialize;

use crate::{
    mwms::planner::PlannerRequest,
    state::MinehouseState,
    work::pool::{WorkUnitSnapshot, WorkUnitState},
};

#[derive(Debug, Serialize, OaSchema)]
pub struct SystemStatusResponse {
    pub planner_queue: Vec<PlannerQueueItem>,
    pub work_unit_pool: WorkUnitPoolSummary,
    pub work_units: Vec<WorkUnitStatus>,
}

#[derive(Debug, Serialize, OaSchema)]
pub struct PlannerQueueItem {
    pub position: usize,
    pub kind: PlannerRequestKind,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, OaSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlannerRequestKind {
    IndexRegions,
    CycleCount,
    Putaway,
}

#[derive(Debug, Serialize, OaSchema)]
pub struct WorkUnitPoolSummary {
    pub total: usize,
    pub queued: usize,
    pub claimed: usize,
    pub completed: usize,
}

#[derive(Debug, Serialize, OaSchema)]
pub struct WorkUnitStatus {
    pub id: String,
    pub kind: WorkUnitKind,
    pub state: WorkUnitStatusState,
    pub priority: usize,
    pub completed_steps: Option<usize>,
    pub total_steps: Option<usize>,
    pub detail: String,
}

#[derive(Debug, Serialize, OaSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitKind {
    IndexRegion,
    IndexContainer,
    Transfer,
    Craft,
}

#[derive(Debug, Serialize, OaSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitStatusState {
    Queued,
    Claimed,
    Completed,
}

#[oasgen(summary = "Get planner queue and work-unit pool status")]
pub async fn get_system_status(
    State(state): State<Arc<MinehouseState>>,
) -> Json<SystemStatusResponse> {
    let planner_queue = state
        .planner_queue
        .snapshot()
        .into_iter()
        .enumerate()
        .map(|(index, request)| planner_queue_item(index + 1, request))
        .collect();
    let mut work_units: Vec<_> = state
        .pool
        .snapshot()
        .into_iter()
        .map(work_unit_status)
        .collect();
    work_units.sort_by_key(|unit| {
        let state_order = match unit.state {
            WorkUnitStatusState::Claimed => 0,
            WorkUnitStatusState::Queued => 1,
            WorkUnitStatusState::Completed => 2,
        };
        (state_order, unit.priority, unit.id.clone())
    });

    let work_unit_pool = WorkUnitPoolSummary {
        total: work_units.len(),
        queued: work_units
            .iter()
            .filter(|unit| matches!(unit.state, WorkUnitStatusState::Queued))
            .count(),
        claimed: work_units
            .iter()
            .filter(|unit| matches!(unit.state, WorkUnitStatusState::Claimed))
            .count(),
        completed: work_units
            .iter()
            .filter(|unit| matches!(unit.state, WorkUnitStatusState::Completed))
            .count(),
    };

    Json(SystemStatusResponse {
        planner_queue,
        work_unit_pool,
        work_units,
    })
}

fn planner_queue_item(position: usize, request: PlannerRequest) -> PlannerQueueItem {
    let (kind, detail) = match request {
        PlannerRequest::IndexRegions => (PlannerRequestKind::IndexRegions, None),
        PlannerRequest::CycleCount => (PlannerRequestKind::CycleCount, None),
        PlannerRequest::Putaway => (PlannerRequestKind::Putaway, None),
    };
    PlannerQueueItem {
        position,
        kind,
        detail,
    }
}

fn work_unit_status(snapshot: WorkUnitSnapshot) -> WorkUnitStatus {
    let state = match snapshot.state {
        WorkUnitState::Queued => WorkUnitStatusState::Queued,
        WorkUnitState::Claimed => WorkUnitStatusState::Claimed,
        WorkUnitState::Completed => WorkUnitStatusState::Completed,
    };
    let (kind, completed_steps, total_steps, detail) = match snapshot.work_unit.data {
        WorkUnitData::IndexRegion(unit) => (
            WorkUnitKind::IndexRegion,
            Some(unit.output_chunks_scanned.len()),
            Some(unit.region_range.covered_chunks().len()),
            format!("{} containers found", unit.output_containers.len()),
        ),
        WorkUnitData::IndexContainer(unit) => (
            WorkUnitKind::IndexContainer,
            Some(usize::from(unit.output.is_some())),
            Some(1),
            format!(
                "container {} at {}, {}, {}",
                unit.container_id, unit.position.x, unit.position.y, unit.position.z
            ),
        ),
        WorkUnitData::Transfer(unit) => (
            WorkUnitKind::Transfer,
            Some(unit.completed.len()),
            Some(unit.transfer.moves.len()),
            format!(
                "{} moves, {} staged",
                unit.transfer.moves.len(),
                unit.staged.len()
            ),
        ),
        WorkUnitData::Craft(unit) => (
            WorkUnitKind::Craft,
            Some(unit.progress.completed_crafts as usize),
            Some(unit.recipe.crafts as usize),
            format!(
                "{} x{}",
                unit.recipe.output_item_kind, unit.recipe.output_yield
            ),
        ),
    };

    WorkUnitStatus {
        id: snapshot.work_unit.id.to_string(),
        kind,
        state,
        priority: snapshot.priority,
        completed_steps,
        total_steps,
        detail,
    }
}
