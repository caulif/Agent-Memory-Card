use crate::jobs::{DesktopTaskStatus, DesktopTaskStore};
use crate::CommandResult;

#[tauri::command]
pub fn get_task_status(task_store: tauri::State<'_, DesktopTaskStore>) -> DesktopTaskStatus {
    task_store.snapshot()
}

#[tauri::command]
pub fn get_job_history(task_store: tauri::State<'_, DesktopTaskStore>) -> Vec<DesktopTaskStatus> {
    task_store.recent_jobs()
}

#[tauri::command]
pub fn cancel_job(
    task_store: tauri::State<'_, DesktopTaskStore>,
    job_id: String,
) -> CommandResult<DesktopTaskStatus> {
    task_store
        .request_cancel(&job_id)
        .ok_or_else(|| format!("job `{job_id}` was not found"))
}
