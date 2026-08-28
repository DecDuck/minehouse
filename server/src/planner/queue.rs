use std::{
    collections::VecDeque,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[repr(transparent)]
pub struct PlannerTaskId(Uuid);

impl PlannerTaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// A high-level warehouse operation the planner turns into concrete work units.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlannerOperation {
    /// Drain a putaway region into category-matched bulk.
    Putaway { region_id: Uuid },
    /// Restock a user pickface region to its configured contents.
    RestockUserPickface { region_id: Uuid },
    /// Keep an order-pickface region stocked with a slice of its categories' bulk.
    SyncOrderPickface { region_id: Uuid },
    /// Re-index a single container's contents.
    IndexContainer { container_id: Uuid },
    /// Fan out reconcile operations for every relevant region.
    ReconcileAll,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlannerTaskStatus {
    Pending,
    Running,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannerTask {
    pub id: PlannerTaskId,
    pub operation: PlannerOperation,
    pub status: PlannerTaskStatus,
    /// Unix-epoch milliseconds when the task was enqueued.
    pub enqueued_at_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlannerQueueSnapshot {
    pub current: Option<PlannerTask>,
    pub pending: Vec<PlannerTask>,
}

struct QueueInner {
    pending: VecDeque<PlannerTask>,
    current: Option<PlannerTask>,
}

/// An inspectable FIFO of planner operations: the API can read the pending list
/// and the currently-running task, unlike an opaque channel.
pub struct PlannerQueue {
    inner: Mutex<QueueInner>,
    notify: Notify,
}

impl PlannerQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(QueueInner {
                pending: VecDeque::new(),
                current: None,
            }),
            notify: Notify::new(),
        }
    }

    /// Enqueues an operation, skipping (returning `None`) when an equal operation
    /// is already pending or currently running.
    pub async fn enqueue(&self, operation: PlannerOperation) -> Option<PlannerTaskId> {
        let mut inner = self.inner.lock().await;
        let duplicate = inner.pending.iter().any(|t| t.operation == operation)
            || inner
                .current
                .as_ref()
                .is_some_and(|c| c.operation == operation);
        if duplicate {
            return None;
        }

        let id = PlannerTaskId::new();
        inner.pending.push_back(PlannerTask {
            id,
            operation,
            status: PlannerTaskStatus::Pending,
            enqueued_at_ms: now_ms(),
        });
        drop(inner);
        self.notify.notify_one();
        Some(id)
    }

    /// Waits for the next operation, marks it running, and returns it.
    pub async fn next(&self) -> PlannerTask {
        loop {
            {
                let mut inner = self.inner.lock().await;
                if let Some(mut task) = inner.pending.pop_front() {
                    task.status = PlannerTaskStatus::Running;
                    inner.current = Some(task.clone());
                    return task;
                }
            }
            self.notify.notified().await;
        }
    }

    pub async fn finish_current(&self) {
        self.inner.lock().await.current = None;
    }

    pub async fn snapshot(&self) -> PlannerQueueSnapshot {
        let inner = self.inner.lock().await;
        PlannerQueueSnapshot {
            current: inner.current.clone(),
            pending: inner.pending.iter().cloned().collect(),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
