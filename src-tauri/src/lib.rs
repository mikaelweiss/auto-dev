use std::sync::Mutex;

use tauri::Manager;

mod db;
mod github;
mod polling;
mod sessions;
mod types;
mod worktrees;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub http_client: reqwest::Client,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize database
            let conn = db::open_and_init().expect("Failed to initialize database");

            let state = AppState {
                db: Mutex::new(conn),
                http_client: reqwest::Client::new(),
            };

            // Check auth and start polling in background
            let has_auth = {
                let db = state.db.lock().expect("DB lock failed during setup");
                db::get_auth_token(&db)
                    .ok()
                    .flatten()
                    .is_some()
            };

            let interval = {
                let db = state.db.lock().expect("DB lock failed during setup");
                db::get_app_settings(&db)
                    .map(|s| s.poll_interval_seconds as u64)
                    .unwrap_or(15)
            };

            app.manage(state);

            if has_auth {
                polling::start_polling(app.handle().clone(), interval);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // GitHub auth
            github::github_auth_from_cli,
            github::github_get_auth_status,
            github::github_logout,
            // GitHub repos
            github::github_list_user_repos,
            github::github_add_repo,
            github::github_remove_repo,
            github::github_get_repos,
            github::github_update_repo,
            // GitHub issues
            github::github_fetch_issues,
            github::github_create_issue,
            github::github_add_label,
            github::github_remove_label,
            github::github_post_comment,
            github::github_create_pr,
            github::github_squash_merge,
            github::github_close_issue,
            // Sessions
            sessions::session_start,
            sessions::session_start_implement,
            sessions::session_start_review,
            sessions::session_respond,
            sessions::session_retry,
            sessions::session_stop,
            sessions::session_run_test,
            sessions::session_get_logs,
            sessions::session_cleanup,
            // Settings
            sessions::settings_get,
            sessions::settings_set,
            sessions::prompts_get,
            sessions::prompts_set,
            sessions::set_repo_path,
            sessions::get_repo_path,
            // Caffeinate
            sessions::start_caffeinate,
            sessions::stop_caffeinate,
            // Polling
            polling::force_poll,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
