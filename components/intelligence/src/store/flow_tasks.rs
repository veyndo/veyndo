//! Whether the work tasks that flow jobs created were applied or started executing, as flow execution summaries show them through marketing's records port. Columns checked at compile time.
use veyndo_core::{Result, State};

/// The application and execution state of one task.
pub struct TaskState {
    pub id: String,
    pub applied: bool,
    pub execution_started: bool,
}

/// The states of the tasks among `ids`, in no particular order.
pub async fn states(a: &State, tenant: &str, ids: &[String]) -> Result<Vec<TaskState>> {
    Ok(sqlx::query_as!(TaskState, r#"SELECT id,COALESCE(applied,false) AS "applied!",COALESCE(run_request ? 'execution',false) AS "execution_started!" FROM tasks WHERE tenant=$1 AND id=ANY($2)"#, tenant, ids)
        .fetch_all(&a.db)
        .await?)
}
