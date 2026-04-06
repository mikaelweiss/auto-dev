use std::collections::HashMap;
use std::sync::Mutex;

use tauri::Manager;

mod claude_provider;
mod codex_provider;
mod db;
mod opencode_provider;
mod github;
mod mcp_handler;
mod polling;
mod provider;
mod sdk_types;
mod issue_state;
mod sessions;
mod sleep;
mod types;
mod worktrees;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub http_client: reqwest::Client,
    /// Maps session DB id -> child process PID for active Claude sessions.
    pub active_pids: Mutex<HashMap<i64, u32>>,
    /// Signals received from the MCP callback server, keyed by session DB id.
    pub mcp_signals: mcp_handler::SignalStore,
    /// Port the MCP callback HTTP server is listening on.
    pub mcp_callback_port: u16,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize database
            let conn = db::open_and_init()
                .map_err(|e| format!("Failed to initialize database: {e}"))?;

            // Start MCP callback server on a random port (bind synchronously so
            // we have the port before AppState is constructed).
            let std_listener = std::net::TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("MCP callback server bind failed: {e}"))?;
            let mcp_port = std_listener
                .local_addr()
                .map_err(|e| format!("MCP callback server addr failed: {e}"))?
                .port();
            std_listener
                .set_nonblocking(true)
                .map_err(|e| format!("MCP callback server nonblocking failed: {e}"))?;

            let state = AppState {
                db: Mutex::new(conn),
                http_client: reqwest::Client::new(),
                active_pids: Mutex::new(HashMap::new()),
                mcp_signals: Mutex::new(HashMap::new()),
                mcp_callback_port: mcp_port,
            };

            // Clean up any sessions left running from a previous app launch
            {
                let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
                let _ = db::fail_orphaned_sessions(&db);
            }

            // Check auth and start polling in background
            let has_auth = {
                let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
                db::get_auth_token(&db)
                    .ok()
                    .flatten()
                    .is_some()
            };

            let interval = {
                let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
                db::get_app_settings(&db)
                    .map(|s| s.poll_interval_seconds as u64)
                    .unwrap_or(15)
            };

            app.manage(state);

            // Start the MCP callback accept loop now that AppState is registered.
            // We pass the std listener and convert to tokio inside the spawned task,
            // because the Tokio reactor isn't available yet in setup().
            tauri::async_runtime::spawn(mcp_handler::run_callback_server(
                std_listener,
                app.handle().clone(),
            ));

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
            github::github_add_local_repo,
            github::github_remove_repo,
            github::github_get_repo_removal_info,
            github::github_get_repos,
            github::github_update_repo,
            // GitHub collaborators
            github::github_list_collaborators,
            // GitHub issues
            github::github_fetch_issues,
            github::github_create_issue,
            github::github_post_comment,
            github::github_create_pr,
            github::github_squash_merge,
            github::github_close_issue,
            github::github_update_issue_body,
            // Sessions
            sessions::session_list,
            sessions::session_start,
            sessions::session_start_implement,
            sessions::session_start_review,
            sessions::session_respond,
            sessions::session_retry,
            sessions::session_stop,
            sessions::session_list_files,
            sessions::session_run_test,
            sessions::session_get_logs,
            sessions::session_cleanup,
            sessions::session_hide,
            sessions::session_unhide,
            sessions::session_list_hidden,
            // Settings & Models
            sessions::settings_get,
            sessions::settings_set,
            sessions::prompts_get,
            sessions::prompts_set,
            sessions::prompts_reset,
            sessions::list_models,
            sessions::set_repo_path,
            sessions::get_repo_path,
            sessions::get_selected_repo_id,
            sessions::set_selected_repo_id,
            // Issue State
            issue_state::get_issue_states,
            issue_state::set_issue_column,
            // Polling
            polling::force_poll,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                sleep::force_disable();
            }
        });
}
