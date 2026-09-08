use uuid::Uuid;

use crate::mwms::crafting::{CraftJobState, CraftJobStatus};

#[derive(Debug, sqlx::FromRow)]
struct CraftJobRow {
    id: Uuid,
    target_item_kind: String,
    target_quantity: i32,
    selections: serde_json::Value,
    state: String,
    completed_quantity: i32,
    error: Option<String>,
}

impl super::DatabaseHandle {
    pub async fn create_craft_job(&self, status: &CraftJobStatus) -> Result<(), sqlx::Error> {
        let id = Uuid::parse_str(&status.id)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?;
        sqlx::query!(
            "insert into craft_job (id, target_item_kind, target_quantity, selections, state, completed_quantity, error) values ($1, $2, $3, $4, $5, $6, $7)",
            id,
            &status.target_item_kind,
            i32::try_from(status.target_quantity).map_err(|_| sqlx::Error::Protocol("craft quantity is too large".into()))?,
            serde_json::to_value(&status.selections).map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
            job_state_name(&status.state),
            i32::try_from(status.completed_quantity).map_err(|_| sqlx::Error::Protocol("completed quantity is too large".into()))?,
            status.error.as_deref(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_craft_job(
        &self,
        id: Uuid,
        state: CraftJobState,
        completed_quantity: u32,
        error: Option<String>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "update craft_job set state = $2, completed_quantity = $3, error = $4, updated_at = now() where id = $1",
            id,
            job_state_name(&state),
            i32::try_from(completed_quantity).map_err(|_| sqlx::Error::Protocol("completed quantity is too large".into()))?,
            error,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fetch_craft_jobs(&self) -> Result<Vec<CraftJobStatus>, sqlx::Error> {
        let rows = sqlx::query_as!(
            CraftJobRow,
            "select id, target_item_kind, target_quantity, selections, state, completed_quantity, error from craft_job order by created_at desc",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(craft_job_status).collect()
    }

    pub async fn fetch_craft_job(&self, id: Uuid) -> Result<Option<CraftJobStatus>, sqlx::Error> {
        let row = sqlx::query_as!(
            CraftJobRow,
            "select id, target_item_kind, target_quantity, selections, state, completed_quantity, error from craft_job where id = $1",
            id,
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(craft_job_status).transpose()
    }
}

fn job_state_name(state: &CraftJobState) -> &'static str {
    match state {
        CraftJobState::Queued => "queued",
        CraftJobState::Waiting => "waiting",
        CraftJobState::Running => "running",
        CraftJobState::Completed => "completed",
        CraftJobState::Failed => "failed",
    }
}

fn craft_job_status(row: CraftJobRow) -> Result<CraftJobStatus, sqlx::Error> {
    let state = match row.state.as_str() {
        "queued" => CraftJobState::Queued,
        "waiting" => CraftJobState::Waiting,
        "running" => CraftJobState::Running,
        "completed" => CraftJobState::Completed,
        "failed" => CraftJobState::Failed,
        state => {
            return Err(sqlx::Error::Protocol(
                format!("unknown craft job state {state}").into(),
            ));
        }
    };
    Ok(CraftJobStatus {
        id: row.id.to_string(),
        target_item_kind: row.target_item_kind,
        target_quantity: u32::try_from(row.target_quantity).map_err(|_| {
            sqlx::Error::Protocol("craft target quantity cannot be negative".into())
        })?,
        selections: serde_json::from_value(row.selections)
            .map_err(|error| sqlx::Error::Protocol(error.to_string().into()))?,
        state,
        completed_quantity: u32::try_from(row.completed_quantity)
            .map_err(|_| sqlx::Error::Protocol("completed quantity cannot be negative".into()))?,
        error: row.error,
    })
}
