//! Convenience wrappers that open their own tenant-scoped transaction. Use the free functions
//! with your own transaction when several steps must commit together.

use factory_accounts::{AccountId, ProjectId, TenantRepo};

use crate::enums::{Actor, TaskState};
use crate::error::TaskError;
use crate::events::{EventDraft, RunEvent, events_after};
use crate::ids::TaskId;
use crate::tasks::{NewTask, Task, create_task, get_task, list_tasks, set_paused, transition_task};

/// The ledger as one account sees it.
#[derive(Clone)]
pub struct Ledger {
    repo: TenantRepo,
}

fn scope_error(e: factory_accounts::ApiError) -> TaskError {
    TaskError::Invalid(format!("could not open a scoped transaction: {}", e.code))
}

impl Ledger {
    pub fn new(repo: TenantRepo) -> Self {
        Self { repo }
    }

    pub fn account_id(&self) -> AccountId {
        self.repo.account_id()
    }

    pub async fn create_task(&self, new: NewTask) -> Result<Task, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        let task = create_task(&mut tx, new).await?;
        tx.commit().await?;
        Ok(task)
    }

    pub async fn get_task(&self, id: TaskId) -> Result<Task, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        get_task(&mut tx, id).await
    }

    pub async fn list_tasks(&self, project: ProjectId) -> Result<Vec<Task>, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        list_tasks(&mut tx, project).await
    }

    pub async fn transition_task(
        &self,
        id: TaskId,
        expected_version: i64,
        to: TaskState,
        draft: EventDraft,
    ) -> Result<Task, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        let task = transition_task(&mut tx, id, expected_version, to, draft).await?;
        tx.commit().await?;
        Ok(task)
    }

    pub async fn set_paused(
        &self,
        id: TaskId,
        expected_version: i64,
        paused: bool,
        reason: Option<&str>,
        actor: Actor,
    ) -> Result<Task, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        let task = set_paused(&mut tx, id, expected_version, paused, reason, actor).await?;
        tx.commit().await?;
        Ok(task)
    }

    pub async fn events_after(
        &self,
        id: TaskId,
        after_seq: i64,
        limit: i64,
    ) -> Result<Vec<RunEvent>, TaskError> {
        let mut tx = self.repo.begin().await.map_err(scope_error)?;
        events_after(&mut tx, id, after_seq, limit).await
    }
}
