use crate::db;
use crate::AppState;

#[tauri::command]
pub async fn get_issue_states(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
) -> Result<Vec<(i64, String)>, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::get_issue_states_for_repo(&db, repo_id)
}

#[tauri::command]
pub async fn set_issue_column(
    state: tauri::State<'_, AppState>,
    repo_id: i64,
    issue_number: i64,
    column_id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    db::set_issue_column(&db, repo_id, issue_number, &column_id)
}
