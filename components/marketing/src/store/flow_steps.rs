//! The per-node execution record of a flow job: an action runs once, and an unfinished record stays uncertain. Columns checked at compile time.
use serde_json::Value;
use veyndo_core::{Result, State};

/// A node's recorded state and result.
pub struct Step {
    pub state: String,
    pub result: Option<Value>,
}

/// The record of `node` in `job`, if the node started before.
pub async fn find(a: &State, job: &str, node: &str) -> Result<Option<Step>> {
    Ok(sqlx::query_as!(
        Step,
        "SELECT state,result FROM flow_steps WHERE job=$1 AND node=$2",
        job,
        node
    )
    .fetch_optional(&a.db)
    .await?)
}

/// Records that `node` starts now.
pub async fn start(a: &State, job: &str, node: &str) -> Result<()> {
    sqlx::query!("INSERT INTO flow_steps(job,node) VALUES($1,$2)", job, node)
        .execute(&a.db)
        .await?;
    Ok(())
}

/// Records the node's result.
pub async fn complete(a: &State, job: &str, node: &str, result: &Value) -> Result<()> {
    sqlx::query!(
        "UPDATE flow_steps SET state='completed',result=$1,finished_at=now() WHERE job=$2 AND node=$3",
        result,
        job,
        node
    )
    .execute(&a.db)
    .await?;
    Ok(())
}

/// Records the node's failure.
pub async fn fail(a: &State, job: &str, node: &str, error: &str) -> Result<()> {
    sqlx::query!(
        "UPDATE flow_steps SET state='failed',error=$1,finished_at=now() WHERE job=$2 AND node=$3",
        error,
        job,
        node
    )
    .execute(&a.db)
    .await?;
    Ok(())
}
