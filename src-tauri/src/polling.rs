use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Manager};

use crate::db;
use crate::github;
use crate::types::*;
use crate::AppState;

/// Shared flag to stop the polling task.
static POLLING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Start the background polling task.
pub fn start_polling(app_handle: tauri::AppHandle, interval_secs: u64) {
    // If already polling, do nothing
    if POLLING_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);

        while POLLING_ACTIVE.load(Ordering::SeqCst) {
            let state = app_handle.state::<AppState>();

            // Read DB data synchronously (no await while lock is held)
            let poll_data = {
                let db = state.db.lock().map_err(|e| format!("DB lock error: {e}"));
                match db {
                    Ok(db) => {
                        let token = db::get_auth_token(&db)
                            .ok()
                            .flatten()
                            .map(|(t, _)| t);
                        let repos = db::get_all_repos(&db).ok().unwrap_or_default();
                        token.map(|t| (t, repos))
                    }
                    Err(e) => {
                        eprintln!("Polling: {e}");
                        None
                    }
                }
            };

            if let Some((token, repos)) = poll_data {
                for repo in &repos {
                    match github::fetch_issues_for_repo(
                        &state.http_client,
                        &token,
                        &repo.owner,
                        &repo.name,
                    )
                    .await
                    {
                        Ok(issues) => {
                            let _ = app_handle.emit(
                                "issues-updated",
                                IssuesUpdatedEvent {
                                    issues,
                                    repo_owner: repo.owner.clone(),
                                    repo_name: repo.name.clone(),
                                },
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "Polling: Failed to fetch issues for {}/{}: {e}",
                                repo.owner, repo.name
                            );
                        }
                    }
                }
            }

            // Drop the state reference before sleeping
            tokio::time::sleep(interval).await;
        }
    });
}

/// Stop the background polling task.
pub fn stop_polling() {
    POLLING_ACTIVE.store(false, Ordering::SeqCst);
}

/// Force an immediate poll cycle (Tauri command).
#[tauri::command]
pub async fn force_poll(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let (token, repos) = {
        let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        let token = db::get_auth_token(&db)?
            .map(|(t, _)| t)
            .ok_or("Not authenticated")?;
        let repos = db::get_all_repos(&db)?;
        (token, repos)
    };

    for repo in &repos {
        match github::fetch_issues_for_repo(&state.http_client, &token, &repo.owner, &repo.name)
            .await
        {
            Ok(issues) => {
                let _ = app_handle.emit(
                    "issues-updated",
                    IssuesUpdatedEvent {
                        issues,
                        repo_owner: repo.owner.clone(),
                        repo_name: repo.name.clone(),
                    },
                );
            }
            Err(e) => {
                eprintln!(
                    "Force poll: Failed to fetch issues for {}/{}: {e}",
                    repo.owner, repo.name
                );
            }
        }
    }

    Ok(())
}
